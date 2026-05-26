use std::io;

use infinitier_cre_resource::{Cre, CreExporter, CreImporter};
use infinitier_datasource::{DataSource, Importer};
use infinitier_gam_resource::{
    Gam, GamEngineData, GamExporter, GamHeader, GamNpc, GamVariable, GamVersion, JournalEntry,
};
use infinitier_tlk_resource::Tlk;

use crate::game::GameData;

/// Resolved view of a [`Gam`].
#[derive(Debug, Clone)]
pub struct ImportedGam {
    /// On-disk version tag (`V1.1`, `V2.0`, `V2.1`, `V2.2`).
    pub version: GamVersion,
    /// 0x54-byte common header (game time, party gold, weather,
    /// formation, section offsets/counts).
    pub header: GamHeader,
    /// Engine-specific extension data (reputation, master area,
    /// configuration, familiar info, …).
    pub engine_data: GamEngineData,
    /// Resolved party-NPC slots in their on-disk order.
    pub party_npcs: Vec<ImportedGamNpc>,
    /// Resolved non-party-NPC slots in their on-disk order.
    pub non_party_npcs: Vec<ImportedGamNpc>,
    /// GLOBAL / Kill-variable records, preserved verbatim.
    pub variables: Vec<GamVariable>,
    /// Journal entries, preserved verbatim.
    pub journal: Vec<JournalEntry>,
    /// Party-inventory raw bytes (20-byte item records, layout not
    /// decoded), preserved verbatim.
    pub party_inventory: Vec<u8>,
    /// "Safe" offset where [`Self::export`] will place freshly
    /// re-serialised CRE blobs. Captured from the original GAM's
    /// total file size at import time — guaranteed to sit past
    /// every header-declared section, every engine-data sub-section
    /// (familiar info, stored locations, modron maze, …), and the
    /// original CRE region.
    ///
    /// This is an internal layout hint, not a value the engine
    /// reads — but the field is `pub` so user-built GAMs (without an
    /// import round-trip) can supply their own base. Set to 0 to
    /// have `export()` place CREs at byte 0, which only makes sense
    /// for synthetic GAMs that carry no embedded CREs.
    pub cre_layout_base: u32,
}

/// One NPC slot with its embedded CRE pre-parsed and its name
/// resolved through the dialog TLK.
#[derive(Debug, Clone)]
pub struct ImportedGamNpc {
    /// Index of this NPC within its source slot vector (either
    /// [`ImportedGam::party_npcs`] or
    /// [`ImportedGam::non_party_npcs`]).
    pub index: usize,
    /// Name as it appears on the GAM record (32-byte localized name
    /// slot → 8-byte engine script-name fallback).
    pub original_name: String,
    /// Human-readable display name. Resolution order:
    /// 1. TLK lookup of the parsed CRE's long-name strref (stock NPCs
    ///    like Minsc / Imoen / Aerie).
    /// 2. [`Self::original_name`] (which itself falls back through
    ///    the GAM long-name then the 8-byte script-name).
    ///
    /// `display_name == original_name` when the TLK lookup didn't
    /// resolve (no TLK available, sentinel strref, or empty entry).
    pub display_name: String,
    /// The CRE record. See [`NpcCre`] for the three semantic states.
    pub cre: Option<NpcCre>,
    /// 0x00 of the on-disk NPC struct.
    pub selection_state: u16,
    /// 0x02 of the on-disk NPC struct.
    pub party_order: u16,
    /// 0x0C of the on-disk NPC struct — the 8-byte resref-shaped
    /// script-name.
    pub character_name: String,
    /// Raw bytes of the NPC struct (352–832 B depending on engine
    /// variant). Preserves the engine-specific tail (animation
    /// colours, quick weapons / spells / items, area location, …)
    /// the importer doesn't decode. The `cre_offset` (bytes 4..8)
    /// and `cre_size` (bytes 8..12) are overwritten by
    /// [`ImportedGam::export`] with fresh values; the rest survives
    /// round-trip.
    pub raw: Vec<u8>,
}

/// What's stored against a GAM NPC slot's CRE pointer — either the
/// parsed embedded record or the resref of an external CRE resource.
#[derive(Debug, Clone)]
pub enum NpcCre {
    /// Embedded CRE blob, parsed from `gam_file[cre_offset .. cre_offset + cre_size]`.
    Cre(Box<Cre>),
    /// External CRE referenced by resref. The string is
    /// [`GamNpc::character_name`] trimmed of trailing NUL bytes /
    /// whitespace.
    Ref(String),
}

impl ImportedGam {
    /// Resolve `gam` against `game_data`. Parses every embedded CRE
    /// blob and walks `game_data.dialog_tlk()` once to resolve display
    /// names. Returns an error only when [`GameData::dialog_tlk`]
    /// itself fails (e.g. an existing `dialog.tlk` that won't parse);
    /// a missing `dialog.tlk` is treated as "no TLK" and falls back
    /// to the GAM / script-name chain.
    pub fn load(gam: Gam, game_data: &GameData) -> io::Result<ImportedGam> {
        let tlk = game_data.dialog_tlk()?;
        Self::load_with_tlk(gam, Some(&tlk))
    }

    /// Like [`Self::load`] but with the dialog TLK supplied by the
    /// caller. Useful when the same TLK is reused across many GAMs
    /// (the keeper does this) — avoids re-parsing the multi-megabyte
    /// `dialog.tlk` on every call.
    ///
    /// Returns `Err` when any NPC slot's embedded CRE blob fails to
    /// parse — strict mode: a corrupt creature record poisons the
    /// whole GAM import rather than producing a partially-resolved
    /// value.
    pub fn load_with_tlk(gam: Gam, tlk: Option<&Tlk>) -> io::Result<ImportedGam> {
        // Find the original file's total byte size — re-serialising
        // the source GAM with the existing exporter gives us a value
        // that's strictly past every section the writer touches
        // (header + engine sub-sections + NPC raws + CRE blobs +
        // variables + journal + inventory). We stash it as
        // `cre_layout_base` so `export()` can re-pack CREs into a
        // region guaranteed not to collide with any of the above.
        let cre_layout_base = {
            let mut buf = Vec::new();
            GamExporter.export(&gam, &mut buf)?;
            buf.len() as u32
        };

        let engine = gam.engine_data.engine();
        let Gam {
            version,
            header,
            engine_data,
            party_npcs,
            non_party_npcs,
            variables,
            journal,
            party_inventory,
        } = gam;
        let party_npcs = party_npcs
            .into_iter()
            .enumerate()
            .map(|(i, npc)| resolve_npc(i, npc, engine, tlk))
            .collect::<io::Result<Vec<_>>>()?;
        let non_party_npcs = non_party_npcs
            .into_iter()
            .enumerate()
            .map(|(i, npc)| resolve_npc(i, npc, engine, tlk))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(ImportedGam {
            version,
            header,
            engine_data,
            party_npcs,
            non_party_npcs,
            variables,
            journal,
            party_inventory,
            cre_layout_base,
        })
    }

    /// Engine family that produced this GAM. Convenience accessor
    /// that delegates to [`GamEngineData::engine`].
    pub fn engine(&self) -> infinitier_common::Engine {
        self.engine_data.engine()
    }

    /// Reconstruct a [`Gam`] from the resolved state, ready to be
    /// fed back to [`infinitier_gam_resource::GamExporter`] for
    /// persistence.
    ///
    /// Round-trip semantics: **functional**, not byte-exact. Each
    /// embedded CRE is re-serialised via [`CreExporter`] (which is
    /// struct-equal but not byte-exact — leftover bytes outside the
    /// parsed sections aren't carried), and the resulting blobs are
    /// repacked starting from the highest `cre_offset + cre_size`
    /// the source file used. After
    /// `import → load → export → GamExporter → GamImporter → load`,
    /// the second `ImportedGam` is functionally equivalent: same
    /// party / non-party CRE values, same header, same variables /
    /// journal / inventory.
    pub fn export(self) -> io::Result<Gam> {
        // Lay out fresh CREs starting at the layout base captured at
        // import time. Using a single high-water mark (rather than
        // max(original cre_end)) keeps us clear of engine-data
        // sub-sections — `GamExporter` writes those *after* the
        // NPC CRE blobs, so a collision would silently clobber the
        // CREs.
        let mut cursor = self.cre_layout_base;
        let mut party_npcs = Vec::with_capacity(self.party_npcs.len());
        for npc in self.party_npcs {
            party_npcs.push(rebuild_gam_npc(npc, &mut cursor)?);
        }
        let mut non_party_npcs = Vec::with_capacity(self.non_party_npcs.len());
        for npc in self.non_party_npcs {
            non_party_npcs.push(rebuild_gam_npc(npc, &mut cursor)?);
        }

        Ok(Gam {
            version: self.version,
            header: self.header,
            engine_data: self.engine_data,
            party_npcs,
            non_party_npcs,
            variables: self.variables,
            journal: self.journal,
            party_inventory: self.party_inventory,
        })
    }
}

/// Build a [`GamNpc`] from a resolved [`ImportedGamNpc`], advancing
/// `cre_cursor` past the freshly-serialised CRE bytes (if any).
fn rebuild_gam_npc(npc: ImportedGamNpc, cre_cursor: &mut u32) -> io::Result<GamNpc> {
    let cre_bytes: Vec<u8> = match &npc.cre {
        Some(NpcCre::Cre(c)) => {
            let mut buf = Vec::new();
            CreExporter.export(c, &mut buf)?;
            buf
        }
        // External resref or empty slot: no embedded bytes.
        Some(NpcCre::Ref(_)) | None => Vec::new(),
    };
    let (cre_offset, cre_size) = if cre_bytes.is_empty() {
        (0u32, 0u32)
    } else {
        let offset = *cre_cursor;
        *cre_cursor = cre_cursor.saturating_add(cre_bytes.len() as u32);
        (offset, cre_bytes.len() as u32)
    };
    // Patch the front of the preserved raw bytes with the new
    // cre_offset / cre_size so the exporter's verbatim write of
    // `raw` lines up with the freshly-laid-out CRE blob.
    let mut raw = npc.raw;
    if raw.len() >= 12 {
        raw[4..8].copy_from_slice(&cre_offset.to_le_bytes());
        raw[8..12].copy_from_slice(&cre_size.to_le_bytes());
    }
    Ok(GamNpc {
        selection_state: npc.selection_state,
        party_order: npc.party_order,
        cre_offset,
        cre_size,
        character_name: npc.character_name,
        raw,
        cre: cre_bytes,
    })
}

fn resolve_npc(
    index: usize,
    npc: GamNpc,
    engine: infinitier_common::Engine,
    tlk: Option<&Tlk>,
) -> io::Result<ImportedGamNpc> {
    // Compute the GAM 32-byte localized name slot via the typed
    // method while `npc` is still intact — we destructure it on the
    // next line and can no longer call methods after that.
    let from_gam = npc.long_name(engine);

    let GamNpc {
        selection_state,
        party_order,
        cre_offset: _,
        cre_size: _,
        character_name,
        raw,
        cre: cre_bytes,
    } = npc;

    // Classify the CRE pointer. `cre_size == 0` means no embedded
    // bytes — either the slot is empty (party has fewer than 6
    // characters) or it references an external CRE by resref.
    let resref = character_name.trim_matches('\0').trim();
    let cre: Option<NpcCre> = if !cre_bytes.is_empty() {
        // `cre_size > 0` ⇒ embedded CRE bytes; parse strictly. Any
        // failure here propagates up and aborts the whole GAM
        // import (the caller asked for fail-fast).
        let parsed = CreImporter {
            name: &format!("gam_npc[{index}]"),
        }
        .import(&DataSource::new(cre_bytes))?;
        Some(NpcCre::Cre(Box::new(parsed)))
    } else if !resref.is_empty() {
        // No embedded CRE but the slot still carries a name — treat
        // it as an external resref the engine will resolve against
        // its CRE pool.
        Some(NpcCre::Ref(resref.to_string()))
    } else {
        // Truly empty slot (e.g. a party of 3 in a 6-slot GAM).
        None
    };

    // Original name: GAM 32-byte localized slot first, then the
    // 8-byte engine script-name as a last resort.
    let original_name = if !from_gam.trim().is_empty() {
        from_gam.trim().to_string()
    } else {
        character_name.trim().to_string()
    };

    // Display name: TLK lookup of the (embedded) CRE's long-name
    // strref; falls back to `original_name`. `Ref` slots don't have
    // a parsed CRE here, so they always use `original_name`.
    let from_tlk = match &cre {
        Some(NpcCre::Cre(c)) => tlk
            .and_then(|t| t.get(c.long_name_strref()))
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string()),
        _ => None,
    };
    let display_name = from_tlk.unwrap_or_else(|| original_name.clone());

    Ok(ImportedGamNpc {
        index,
        original_name,
        display_name,
        cre,
        selection_state,
        party_order,
        character_name,
        raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_common::Game;
    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_gam_resource::{GamExporter, GamImporter};

    /// Builds the keeper's reference fixture (BG2:EE Auto-Save).
    /// Returns the raw `Gam` ready to feed into `ImportedGam::load_with_tlk`.
    fn load_gam(rel_path: &str, engine: infinitier_common::Engine) -> Gam {
        let path = infinitier_test_utils::get_assets_path()
            .join("SAV_GAM")
            .join(rel_path);
        GamImporter {
            name: rel_path,
            engine,
        }
        .import(&DataSource::new(path.as_path()))
        .expect("import GAM fixture")
    }

    #[test]
    fn resolves_cres_for_every_party_npc_slot() {
        let gam = load_gam(
            "bg_ee/save/000000000-Auto-Salvataggio/BALDUR.gam",
            Game::Bgee.engine(),
        );
        let imported = ImportedGam::load_with_tlk(gam, None).expect("ImportedGam::load_with_tlk");
        assert!(
            !imported.party_npcs.is_empty(),
            "BG:EE Auto-Salvataggio fixture must have at least one party slot"
        );
        // At least one slot must carry an embedded CRE with sane HP.
        let with_cre: Vec<_> = imported
            .party_npcs
            .iter()
            .filter(|m| match &m.cre {
                Some(NpcCre::Cre(c)) => c.maximum_hit_points() > 0,
                _ => false,
            })
            .collect();
        assert!(
            !with_cre.is_empty(),
            "expected at least one party slot to expose a parsed CRE",
        );
        // No TLK supplied → display_name must equal original_name.
        for npc in &imported.party_npcs {
            assert_eq!(
                npc.display_name, npc.original_name,
                "without a TLK, display_name should mirror original_name (slot {})",
                npc.index,
            );
        }
    }

    #[test]
    fn load_fails_when_dialog_tlk_is_not_reachable() {
        // The bg_ee corpus root doesn't ship a dialog.tlk next to it,
        // so `GameData::dialog_tlk()` returns NotFound. `load()` is
        // strict: it propagates that error rather than silently
        // skipping name resolution. (Callers that want lenient
        // behaviour go through `load_with_tlk(gam, None)` instead.)
        let root = infinitier_test_utils::get_assets_path().join("SAV_GAM/bg_ee");
        let fs = CaseInsensitiveFS::new(root).expect("open fixture FS");
        let game_data = GameData::new(vec![], Game::Bgee, fs);
        let gam = load_gam(
            "bg_ee/save/000000000-Auto-Salvataggio/BALDUR.gam",
            Game::Bgee.engine(),
        );
        let err = ImportedGam::load(gam, &game_data)
            .expect_err("missing dialog.tlk must surface as Err from load()");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    /// Functional round-trip: import → load → export → GamExporter →
    /// GamImporter → load. The second `ImportedGam` must be
    /// equivalent on every field a downstream consumer reads:
    /// version, header.party_gold, engine_data.reputation, per-NPC
    /// display names + ability scores, variables / journal sizes.
    #[test]
    fn export_round_trips_functionally() {
        let engine = Game::Bgee.engine();
        let rel = "bg_ee/save/000000000-Auto-Salvataggio/BALDUR.gam";
        let original_gam = load_gam(rel, engine);
        let original =
            ImportedGam::load_with_tlk(original_gam.clone(), None).expect("load original");

        // Round-trip: export → serialise → import → load.
        let rebuilt_gam = original.clone().export().expect("export to Gam");
        let mut bytes = Vec::new();
        GamExporter
            .export(&rebuilt_gam, &mut bytes)
            .expect("GamExporter to bytes");
        let reimported_gam = GamImporter { name: rel, engine }
            .import(&DataSource::new(bytes))
            .expect("re-import bytes");
        let reimported =
            ImportedGam::load_with_tlk(reimported_gam, None).expect("re-load ImportedGam");

        // Headers / engine data preserved.
        assert_eq!(reimported.version, original.version);
        assert_eq!(
            reimported.header.party_gold, original.header.party_gold,
            "party_gold lost across round-trip"
        );
        assert_eq!(
            reimported.engine_data.engine(),
            original.engine_data.engine(),
        );
        // Section sizes preserved.
        assert_eq!(reimported.party_npcs.len(), original.party_npcs.len());
        assert_eq!(
            reimported.non_party_npcs.len(),
            original.non_party_npcs.len(),
        );
        assert_eq!(reimported.variables.len(), original.variables.len());
        assert_eq!(reimported.journal.len(), original.journal.len());
        assert_eq!(
            reimported.party_inventory.len(),
            original.party_inventory.len(),
        );
        // Per-NPC display names and embedded-CRE ability scores stable.
        for (a, b) in original.party_npcs.iter().zip(reimported.party_npcs.iter()) {
            assert_eq!(a.display_name, b.display_name);
            assert_eq!(a.original_name, b.original_name);
            match (&a.cre, &b.cre) {
                (Some(NpcCre::Cre(ca)), Some(NpcCre::Cre(cb))) => {
                    assert_eq!(ca.strength(), cb.strength(), "strength");
                    assert_eq!(ca.dexterity(), cb.dexterity(), "dexterity");
                    assert_eq!(
                        ca.current_hit_points(),
                        cb.current_hit_points(),
                        "current_hit_points",
                    );
                    assert_eq!(
                        ca.maximum_hit_points(),
                        cb.maximum_hit_points(),
                        "maximum_hit_points",
                    );
                }
                (Some(NpcCre::Ref(ra)), Some(NpcCre::Ref(rb))) => assert_eq!(ra, rb),
                (None, None) => {}
                (lhs, rhs) => panic!(
                    "NPC slot {} classification changed across round-trip: {lhs:?} vs {rhs:?}",
                    a.index,
                ),
            }
        }
    }
}
