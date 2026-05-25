//! Discovery and loading of a save folder.
//!
//! A save folder is a single sub-directory of the engine's `save/`
//! (or `mpsave/`) tree — e.g. `BALDUR.../save/000000001-Quick-Save/`.
//! It always contains exactly one `.GAM` file (the campaign-wide
//! mutable state) plus a `.SAV` file (per-area state) and possibly
//! a few NPC `.CHR` files. The MVP only touches the `.GAM`.

use std::path::{Path, PathBuf};

use infinitier_common::Engine;
use infinitier_datasource::{DataSource, Importer};
use infinitier_gam_resource::{Gam, GamImporter, GamNpc};

use crate::cre::{Abilities, CreSummary, parse_cre};

/// A loaded save game — the parsed [`Gam`] plus a parsed per-party
/// summary so the UI can render without re-walking the CRE blobs on
/// every frame.
#[derive(Debug, Clone)]
pub struct SaveGame {
    /// Absolute path to the loaded save folder.
    pub save_path: PathBuf,
    /// The GAM file we loaded (e.g. `BALDUR.GAM`, `ICEWIND2.GAM`,
    /// `TORMENT.GAM`).
    pub gam_file_name: String,
    /// The parsed save state.
    pub gam: Gam,
    /// One [`PartyMember`] per slot in [`Gam::party_npcs`], pre-parsed
    /// so the UI is dumb.
    pub party: Vec<PartyMember>,
}

/// One party-member row surfaced to the UI. Carries both the
/// human-friendly name (the engine's 8-byte slot label) and the
/// stats parsed out of the embedded CRE blob.
#[derive(Debug, Clone)]
pub struct PartyMember {
    /// Index into [`Gam::party_npcs`] — preserved so future edit code
    /// can map back to the underlying record.
    #[allow(dead_code)]
    pub gam_party_index: usize,
    /// The 8-byte engine-side character name (often the short name),
    /// taken from [`GamNpc::character_name`]. Empty string when the
    /// slot's name is all-NULs.
    pub display_name: String,
    /// Either the parsed CRE summary or a load error message — the
    /// UI still wants to render the row even when parsing the
    /// embedded CRE fails (e.g. cre_size = 0 for an empty slot).
    pub cre: Result<CreSummary, String>,
}

impl PartyMember {
    /// Convenience: returns the abilities if the CRE blob parsed
    /// successfully. Will be used by upcoming edit / search features.
    #[allow(dead_code)]
    pub fn abilities(&self) -> Option<&Abilities> {
        self.cre.as_ref().ok().map(|s| &s.abilities)
    }
}

/// Locates the single `.GAM` file inside `save_dir`. Errors when
/// none is present or when more than one is found.
pub fn find_gam_file(save_dir: &Path) -> std::io::Result<PathBuf> {
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(save_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("gam"))
                .unwrap_or(false)
        {
            candidates.push(path);
        }
    }
    match candidates.len() {
        0 => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "No .GAM file found in save directory '{}'",
                save_dir.display()
            ),
        )),
        1 => Ok(candidates.pop().unwrap()),
        _ => Err(std::io::Error::other(format!(
            "Multiple .GAM files found in save directory '{}': {:?}",
            save_dir.display(),
            candidates,
        ))),
    }
}

/// Loads the save game in `save_dir`, parsing the GAM with the given
/// [`Engine`] (provided by the caller after game detection on the
/// game folder).
pub fn load_save(save_dir: &Path, engine: Engine) -> std::io::Result<SaveGame> {
    let gam_path = find_gam_file(save_dir)?;
    let gam_file_name = gam_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<gam>")
        .to_string();

    let gam: Gam = GamImporter {
        name: &gam_file_name,
        engine,
    }
    .import(&DataSource::new(gam_path.as_path()))?;

    let party = gam
        .party_npcs
        .iter()
        .enumerate()
        .map(|(idx, npc)| build_party_member(idx, npc))
        .collect();

    Ok(SaveGame {
        save_path: save_dir.to_path_buf(),
        gam_file_name,
        gam,
        party,
    })
}

fn build_party_member(gam_party_index: usize, npc: &GamNpc) -> PartyMember {
    let display_name = npc.character_name.trim().to_string();
    let cre_bytes = npc.cre_data();
    let cre = if cre_bytes.is_empty() {
        Err("embedded CRE blob is empty (slot has no creature record)".to_string())
    } else {
        parse_cre(cre_bytes).map_err(|e| e.to_string())
    };
    PartyMember {
        gam_party_index,
        display_name,
        cre,
    }
}
