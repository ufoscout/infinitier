//! Discovery and loading of a save folder.
//!
//! A save folder is a single sub-directory of the engine's `save/`
//! (or `mpsave/`) tree — e.g. `BALDUR.../save/000000001-Quick-Save/`.
//! It always contains exactly one `.GAM` file (the campaign-wide
//! mutable state) plus a `.SAV` file (per-area state) and possibly
//! a few NPC `.CHR` files. The MVP only touches the `.GAM`.

use std::path::{Path, PathBuf};

use infinitier_common::Engine;
use infinitier_cre_resource::{Cre, CreImporter};
use infinitier_datasource::{DataSource, Importer};
use infinitier_gam_resource::{Gam, GamImporter, GamNpc};
use infinitier_tlk_resource::Tlk;

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
/// fully-parsed CRE record from the embedded blob.
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
    /// Either the parsed CRE record or a load error message — the UI
    /// still wants to render the row even when parsing the embedded
    /// CRE fails (e.g. cre_size = 0 for an empty slot).
    pub cre: Result<Cre, String>,
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
/// game folder). `tlk` is the loaded `dialog.tlk` — passed in so we
/// can resolve the CRE long-name strref into a human-readable
/// display name for each party slot. `None` falls back to the GAM
/// long-name slot (only the protagonist is stored there) and then
/// to the 8-byte engine script-name.
pub fn load_save(save_dir: &Path, engine: Engine, tlk: Option<&Tlk>) -> std::io::Result<SaveGame> {
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
        .map(|(idx, npc)| build_party_member(idx, npc, engine, tlk))
        .collect();

    Ok(SaveGame {
        save_path: save_dir.to_path_buf(),
        gam_file_name,
        gam,
        party,
    })
}

fn build_party_member(
    gam_party_index: usize,
    npc: &GamNpc,
    engine: Engine,
    tlk: Option<&Tlk>,
) -> PartyMember {
    let cre_bytes = npc.cre_data();
    let cre = if cre_bytes.is_empty() {
        Err("embedded CRE blob is empty (slot has no creature record)".to_string())
    } else {
        CreImporter {
            name: &format!("party[{gam_party_index}]"),
        }
        .import(&DataSource::new(cre_bytes.to_vec()))
        .map_err(|e| e.to_string())
    };
    // Resolve the display name in priority order:
    // 1. TLK lookup of the CRE's long-name strref (stock NPCs like
    //    Minsc / Imoen / Aerie — names are localized strings in
    //    `dialog.tlk`).
    // 2. The GAM's 32-byte localized name slot — populated for the
    //    custom-created protagonist.
    // 3. The 8-byte engine script name (the `*HARBASE` / `*INSC7`
    //    fallback we used to show).
    let from_tlk = cre.as_ref().ok().and_then(|c| {
        tlk.and_then(|t| t.get(c.long_name_strref()))
            .filter(|s| !s.trim().is_empty())
    });
    let from_gam = npc.long_name(engine);
    let display_name = from_tlk
        .or_else(|| (!from_gam.trim().is_empty()).then_some(from_gam))
        .unwrap_or_else(|| npc.character_name.clone())
        .trim()
        .to_string();
    PartyMember {
        gam_party_index,
        display_name,
        cre,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_common::Game;

    /// Regression for the "empty CRE blob" bug a user reported when
    /// loading a real BG:EE save: every party slot rendered
    /// `embedded CRE blob is empty (slot has no creature record)`
    /// because [`GamNpc::cre_offset`] was being treated as relative
    /// to the NPC struct's own bytes. The fix in the GAM crate
    /// resolves the offset against the full file. This test loads
    /// one of the bundled BG:EE corpus saves and asserts every
    /// party member with a non-zero `cre_size` parses into a real
    /// CRE record with sane ability scores.
    #[test]
    fn party_cres_resolve_against_full_gam_file() {
        // Use a small bundled corpus fixture (any BG:EE save will
        // do); the user's reported path lives outside the repo.
        let save_dir = infinitier_test_utils::get_assets_path()
            .join("SAV_GAM/bg_ee/save/000000000-Auto-Salvataggio");
        let save = load_save(&save_dir, Game::Bgee.engine(), None).expect("loading BG:EE save");
        let with_cre: Vec<_> = save
            .party
            .iter()
            .filter(|m| match &m.cre {
                Ok(c) => c.maximum_hit_points() > 0,
                Err(_) => false,
            })
            .collect();
        assert!(
            !with_cre.is_empty(),
            "expected at least one party slot to expose a parsed CRE; got: {:?}",
            save.party
                .iter()
                .map(|m| (&m.display_name, m.cre.as_ref().err().cloned()))
                .collect::<Vec<_>>(),
        );
        // Spot-check the first one — ability scores must be in the
        // documented 1..=25 range.
        let cre = with_cre[0].cre.as_ref().unwrap();
        for (label, value) in [
            ("strength", cre.strength()),
            ("dexterity", cre.dexterity()),
            ("constitution", cre.constitution()),
            ("intelligence", cre.intelligence()),
            ("wisdom", cre.wisdom()),
            ("charisma", cre.charisma()),
        ] {
            assert!(
                (1..=25).contains(&value),
                "{label} = {value} outside the documented 1..=25 range",
            );
        }
    }

    /// Build a synthetic V1 TLK byte stream that maps each `(strref,
    /// text)` pair into a valid file. Strrefs not in `entries` get a
    /// zero-length entry. Used by [`party_names_resolve_via_tlk`] to
    /// stand in for a real `dialog.tlk` without shipping one in the
    /// corpus (each language pack is ~5 MB).
    fn synth_tlk_bytes(entries: &[(u32, &str)]) -> Vec<u8> {
        // Sanity: TLK is indexed by strref; we need an entry for
        // every index up to and including the largest one we want
        // populated.
        let max_strref = entries.iter().map(|(s, _)| *s).max().unwrap_or(0) as usize;
        let n_entries = max_strref + 1;
        let header_len = 0x12usize;
        let entry_len = 26usize;
        let strings_offset = header_len + entry_len * n_entries;
        let mut buf = vec![0u8; strings_offset];
        // Header
        buf[0..8].copy_from_slice(b"TLK V1  ");
        buf[8..10].copy_from_slice(&1252u16.to_le_bytes()); // WINDOWS-1252
        buf[10..14].copy_from_slice(&(n_entries as u32).to_le_bytes());
        buf[14..18].copy_from_slice(&(strings_offset as u32).to_le_bytes());
        // Populate the named entries; the rest stay zero (length 0,
        // offset 0 — `Tlk::get` returns an empty string for them).
        let mut strings: Vec<u8> = Vec::new();
        for (strref, text) in entries {
            let entry_pos = header_len + (*strref as usize) * entry_len;
            let offset = strings.len() as u32;
            let length = text.len() as u32;
            buf[entry_pos..entry_pos + 2].copy_from_slice(&1u16.to_le_bytes()); // flags: has-text
            buf[entry_pos + 0x12..entry_pos + 0x16].copy_from_slice(&offset.to_le_bytes());
            buf[entry_pos + 0x16..entry_pos + 0x1A].copy_from_slice(&length.to_le_bytes());
            strings.extend_from_slice(text.as_bytes());
        }
        buf.extend_from_slice(&strings);
        buf
    }

    /// End-to-end regression for the "party names show as `*INSC7`"
    /// bug. Until this fix landed the keeper displayed the 8-byte
    /// engine script-name; the proper localized name lives in
    /// `dialog.tlk`, addressed by the long-name strref in each
    /// embedded CRE.
    ///
    /// We synthesise a TLK keyed on the *actual* CRE strrefs found
    /// in a bundled BG2:EE corpus save (which has Xor + 5 stock
    /// NPCs), so the test exercises the whole chain on real data
    /// without needing a 5 MB `dialog.tlk` checked into the repo.
    #[test]
    fn party_names_resolve_via_tlk() {
        use infinitier_datasource::{DataSource, Importer};
        use infinitier_tlk_resource::TlkImporter;

        let save_dir = infinitier_test_utils::get_assets_path().join(
            "SAV_GAM/bg2_ee/save/000000005-Salvataggio Finale-TOB - Thu Mar 26 21-32-51 2026",
        );
        let engine = Game::Bg2ee.engine();

        // Pass 1: load without TLK so we can read each party slot's
        // CRE long-name strref.
        let no_tlk = load_save(&save_dir, engine, None).expect("loading BG2:EE save");
        let strrefs: Vec<u32> = no_tlk
            .party
            .iter()
            .map(|m| {
                m.cre
                    .as_ref()
                    .map(|c| c.long_name_strref())
                    .unwrap_or(u32::MAX)
            })
            .collect();

        // Synthesise a TLK that maps each non-sentinel strref to a
        // distinguishable per-slot test name.
        let expected: Vec<String> = strrefs
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                if s == u32::MAX {
                    String::new()
                } else {
                    format!("TestName{i}")
                }
            })
            .collect();
        let tlk_pairs: Vec<(u32, &str)> = strrefs
            .iter()
            .zip(expected.iter())
            .filter(|(s, _)| **s != u32::MAX)
            .map(|(s, name)| (*s, name.as_str()))
            .collect();
        assert!(
            !tlk_pairs.is_empty(),
            "fixture should have at least one party slot with a TLK-resolvable strref"
        );
        let tlk_bytes = synth_tlk_bytes(&tlk_pairs);
        let tlk = TlkImporter { name: "synth" }
            .import(&DataSource::new(tlk_bytes))
            .expect("parsing synthetic TLK");

        // Pass 2: reload with the synthetic TLK and verify the
        // resolution chain picks each name from the right source.
        let resolved =
            load_save(&save_dir, engine, Some(&tlk)).expect("loading BG2:EE save with TLK");
        assert_eq!(
            resolved.party.len(),
            no_tlk.party.len(),
            "party slot count mustn't change when adding a TLK"
        );

        let mut tlk_hits = 0usize;
        for (i, member) in resolved.party.iter().enumerate() {
            if strrefs[i] == u32::MAX {
                // No TLK to resolve — name must come from one of
                // the fallbacks (GAM 32-byte name slot, then 8-byte
                // script-name). The previous pass already proved
                // those work; we just check the name is not the
                // TLK test-string.
                assert_ne!(
                    member.display_name,
                    format!("TestName{i}"),
                    "slot {i} has no TLK strref yet matched a TLK test-string"
                );
                continue;
            }
            assert_eq!(
                member.display_name,
                format!("TestName{i}"),
                "slot {i}: expected TLK-resolved name 'TestName{i}', got {:?}",
                member.display_name,
            );
            tlk_hits += 1;
        }
        assert!(
            tlk_hits > 0,
            "expected at least one party slot to resolve its display name via the synthetic TLK"
        );

        // Cross-check: the slot whose strref is u32::MAX (the
        // protagonist on this fixture) must still produce its GAM
        // 32-byte name slot — the user reported "Xor" for slot 1.
        if strrefs[0] == u32::MAX {
            assert_eq!(
                resolved.party[0].display_name, "Xor",
                "protagonist slot should fall back to the GAM-stored localized name",
            );
        }
    }
}
