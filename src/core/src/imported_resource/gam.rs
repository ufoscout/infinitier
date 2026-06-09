use std::io;

use infinitier_common::Game;
use infinitier_cre_resource::{CreExporter, CreImporter};

use super::cre::ImportedCre;
use infinitier_datasource::{DataSource, Importer};
use infinitier_gam_resource::{
    Gam, GamEngineData, GamHeader, GamNpc, GamVariable, GamVersion, JournalEntry, NpcCharStats,
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
    /// The party-member character-statistics block (kill counts, time
    /// in party, favourite spells/weapons, …), parsed by the gam
    /// importer. Round-trips via [`ImportedGam::export`].
    pub char_stats: NpcCharStats,
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
    Cre(ImportedCre),
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
        Self::load_with_tlk(gam, game_data.game(), Some(&tlk))
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
    ///
    /// `game` is needed to parse embedded CREs: a handful of V1.0 header
    /// regions are engine-specific (e.g. the PST:EE overlay of the BG
    /// "tracking target" field), and the CRE version tag alone can't tell
    /// PST:EE apart from the other Enhanced-Edition games.
    pub fn load_with_tlk(gam: Gam, game: Game, tlk: Option<&Tlk>) -> io::Result<ImportedGam> {
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
            .map(|(i, npc)| resolve_npc(i, npc, engine, game, tlk))
            .collect::<io::Result<Vec<_>>>()?;
        let non_party_npcs = non_party_npcs
            .into_iter()
            .enumerate()
            .map(|(i, npc)| resolve_npc(i, npc, engine, game, tlk))
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
    /// parsed sections aren't carried). [`infinitier_gam_resource::GamExporter`] then lays out
    /// the whole file contiguously, computing each CRE blob's offset
    /// and size on the fly, so re-serialised blobs of any size pack
    /// without collision. After
    /// `import → load → export → GamExporter → GamImporter → load`,
    /// the second `ImportedGam` is functionally equivalent: same
    /// party / non-party CRE values, same header, same variables /
    /// journal / inventory.
    pub fn export(self) -> io::Result<Gam> {
        let mut party_npcs = Vec::with_capacity(self.party_npcs.len());
        for npc in self.party_npcs {
            party_npcs.push(rebuild_gam_npc(npc)?);
        }
        let mut non_party_npcs = Vec::with_capacity(self.non_party_npcs.len());
        for npc in self.non_party_npcs {
            non_party_npcs.push(rebuild_gam_npc(npc)?);
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

/// Build a [`GamNpc`] from a resolved [`ImportedGamNpc`]. The embedded
/// CRE blob's file offset and size are no longer computed here —
/// [`infinitier_gam_resource::GamExporter`] places every blob contiguously and patches each
/// NPC record's `raw[0x04..0x0C]` with the recomputed values, so a
/// re-serialised CRE of any size lays out without collision.
fn rebuild_gam_npc(npc: ImportedGamNpc) -> io::Result<GamNpc> {
    let cre_bytes: Vec<u8> = match &npc.cre {
        Some(NpcCre::Cre(c)) => {
            let mut buf = Vec::new();
            CreExporter.export(c, &mut buf)?;
            buf
        }
        // External resref or empty slot: no embedded bytes.
        Some(NpcCre::Ref(_)) | None => Vec::new(),
    };
    Ok(GamNpc {
        selection_state: npc.selection_state,
        party_order: npc.party_order,
        character_name: npc.character_name,
        char_stats: npc.char_stats,
        raw: npc.raw,
        cre: cre_bytes,
    })
}

fn resolve_npc(
    index: usize,
    npc: GamNpc,
    engine: infinitier_common::Engine,
    game: Game,
    tlk: Option<&Tlk>,
) -> io::Result<ImportedGamNpc> {
    // Compute the GAM 32-byte localized name slot via the typed
    // method while `npc` is still intact — we destructure it on the
    // next line and can no longer call methods after that.
    let from_gam = npc.long_name(engine);

    let GamNpc {
        selection_state,
        party_order,
        character_name,
        char_stats,
        raw,
        cre: cre_bytes,
    } = npc;

    // Classify the CRE pointer. An empty `cre` blob means no embedded
    // bytes — either the slot is empty (party has fewer than 6
    // characters) or it references an external CRE by resref.
    let resref = character_name.trim_matches('\0').trim();
    let cre: Option<NpcCre> = if !cre_bytes.is_empty() {
        // `cre_size > 0` ⇒ embedded CRE bytes; parse strictly. Any
        // failure here propagates up and aborts the whole GAM
        // import (the caller asked for fail-fast).
        let parsed = CreImporter {
            name: &format!("gam_npc[{index}]"),
            game,
        }
        .import(&DataSource::new(cre_bytes))?;
        Some(NpcCre::Cre(ImportedCre::new(parsed)))
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
        char_stats,
        raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let imported =
            ImportedGam::load_with_tlk(gam, Game::Bgee, None).expect("ImportedGam::load_with_tlk");
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

    /// Every party member of a multi-character classic-PST save must read as
    /// a fully-parsed, embedded CRE — not an external reference.
    ///
    /// Regression for the PST NPC-record-size bug: PST's GAM record is 360
    /// bytes (not the BG 352), and it has no shared party-inventory section
    /// to derive that from. With the wrong stride only the first member
    /// (Nameless) was read; the rest came back as empty/external slots. This
    /// 5-member "Modron Foyer" save exercises all of them.
    #[test]
    fn pst_every_party_member_is_a_readable_embedded_cre() {
        let gam = load_gam(
            "pst/save/000000029-Modron-Foyer/TORMENT.GAM",
            Game::Pst.engine(),
        );
        let imported =
            ImportedGam::load_with_tlk(gam, Game::Pst, None).expect("ImportedGam::load_with_tlk");
        assert!(
            imported.party_npcs.len() >= 5,
            "Modron Foyer save should carry a full party, got {}",
            imported.party_npcs.len(),
        );
        for npc in &imported.party_npcs {
            match &npc.cre {
                Some(NpcCre::Cre(cre)) => {
                    // A real, parsed creature: non-empty name and sane HP.
                    assert!(
                        cre.maximum_hit_points() > 0,
                        "party slot {} has a CRE but zero max HP — likely misread",
                        npc.index,
                    );
                }
                other => panic!(
                    "party slot {} is not a readable embedded CRE: {other:?}",
                    npc.index,
                ),
            }
        }
    }

    /// Every party member of a full six-character classic Icewind Dale save
    /// must read as a fully-parsed, embedded CRE.
    ///
    /// Regression for the IWD NPC-record-size bug: IWD's GAM record is 384
    /// bytes (not the BG 352), and — like PST — it has no shared
    /// party-inventory section to derive that from. With the wrong stride the
    /// keeper failed to start (a later slot's CRE pointer landed on a
    /// zero-filled region → "Unsupported CRE signature: [0,0,0,0]"). This
    /// 6-member auto-save exercises all of them.
    #[test]
    fn iwd_every_party_member_is_a_readable_embedded_cre() {
        let gam = load_gam(
            "iwd/mpsave/000000000-Auto-Save/ICEWIND.GAM",
            Game::Iwd {
                heart_of_winter: false,
                totl: false,
            }
            .engine(),
        );
        let imported = ImportedGam::load_with_tlk(
            gam,
            Game::Iwd {
                heart_of_winter: false,
                totl: false,
            },
            None,
        )
        .expect("ImportedGam::load_with_tlk");
        assert_eq!(
            imported.party_npcs.len(),
            6,
            "IWD auto-save should carry a full six-character party",
        );
        for npc in &imported.party_npcs {
            match &npc.cre {
                Some(NpcCre::Cre(cre)) => assert!(
                    cre.maximum_hit_points() > 0,
                    "party slot {} has a CRE but zero max HP — likely misread",
                    npc.index,
                ),
                other => panic!(
                    "party slot {} is not a readable embedded CRE: {other:?}",
                    npc.index,
                ),
            }
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
        let original = ImportedGam::load_with_tlk(original_gam.clone(), Game::Bgee, None)
            .expect("load original");

        // Round-trip: export → serialise → import → load.
        let rebuilt_gam = original.clone().export().expect("export to Gam");
        let mut bytes = Vec::new();
        GamExporter
            .export(&rebuilt_gam, &mut bytes)
            .expect("GamExporter to bytes");
        let reimported_gam = GamImporter { name: rel, engine }
            .import(&DataSource::new(bytes))
            .expect("re-import bytes");
        let reimported = ImportedGam::load_with_tlk(reimported_gam, Game::Bgee, None)
            .expect("re-load ImportedGam");

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
