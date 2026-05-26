
use std::io;

use infinitier_cre_resource::{Cre, CreImporter};
use infinitier_datasource::{DataSource, Importer};
use infinitier_gam_resource::{Gam, GamNpc};
use infinitier_tlk_resource::Tlk;

use crate::game::GameData;

/// Resolved view of a [`Gam`].
#[derive(Debug, Clone)]
pub struct ImportedGam {
    /// The underlying parsed GAM file. Preserved verbatim so callers
    /// can still mutate / re-export the on-disk state.
    pub gam: Box<Gam>,
    /// Resolved party-NPC slots, one entry per [`Gam::party_npcs`]
    /// slot, in the same order.
    pub party_npcs: Vec<ImportedGamNpc>,
    /// Resolved non-party-NPC slots, one entry per
    /// [`Gam::non_party_npcs`] slot, in the same order.
    pub non_party_npcs: Vec<ImportedGamNpc>,
}

/// One NPC slot with its embedded CRE pre-parsed and its name
/// resolved through the dialog TLK.
#[derive(Debug, Clone)]
pub struct ImportedGamNpc {
    /// Index of this NPC within its source slot vector (either
    /// [`Gam::party_npcs`] or [`Gam::non_party_npcs`]). Preserved so
    /// callers can map back to the underlying record for edits.
    pub index: usize,
    /// Name as it appears on the GAM record.
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
    /// The parsed embedded CRE, if any.
    pub cre: Option<NpcCre>,
}

/// What's stored against a GAM NPC slot's CRE pointer — either the
/// parsed embedded record or the resref of an external CRE resource.
#[derive(Debug, Clone)]
pub enum NpcCre {
    /// Embedded CRE blob, parsed from `gam_file[cre_offset .. cre_offset + cre_size]`.
    Cre(Cre),
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
        let engine = gam.engine_data.engine();
        let party_npcs = gam
            .party_npcs
            .iter()
            .enumerate()
            .map(|(i, npc)| resolve_npc(i, npc, engine, tlk))
            .collect::<io::Result<Vec<_>>>()?;
        let non_party_npcs = gam
            .non_party_npcs
            .iter()
            .enumerate()
            .map(|(i, npc)| resolve_npc(i, npc, engine, tlk))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(ImportedGam {
            gam: Box::new(gam),
            party_npcs,
            non_party_npcs,
        })
    }
}

fn resolve_npc(
    index: usize,
    npc: &GamNpc,
    engine: infinitier_common::Engine,
    tlk: Option<&Tlk>,
) -> io::Result<ImportedGamNpc> {
    // Classify the CRE pointer. `cre_size == 0` means no embedded
    // bytes — either the slot is empty (party has fewer than 6
    // characters) or it references an external CRE by resref.
    let cre_bytes = npc.cre_data();
    let resref = npc.character_name.trim_matches('\0').trim();
    let cre: Option<NpcCre> = if !cre_bytes.is_empty() {
        // `cre_size > 0` ⇒ embedded CRE bytes; parse strictly. Any
        // failure here propagates up and aborts the whole GAM
        // import (the caller asked for fail-fast).
        let parsed = CreImporter {
            name: &format!("gam_npc[{index}]"),
        }
        .import(&DataSource::new(cre_bytes.to_vec()))?;
        Some(NpcCre::Cre(parsed))
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
    let from_gam = npc.long_name(engine);
    let original_name = if !from_gam.trim().is_empty() {
        from_gam.trim().to_string()
    } else {
        npc.character_name.trim().to_string()
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_common::Game;
    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_gam_resource::GamImporter;

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
            ImportedGam::load_with_tlk(gam, None).expect("ImportedGam::load_with_tlk");
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

}
