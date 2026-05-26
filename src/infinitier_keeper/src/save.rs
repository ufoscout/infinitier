//! Loading of an in-engine save game.
//!
//! Discovery (which save folders exist, where the SAV/GAM/portrait
//! files live) is owned by [`infinitier_core::save_games`]. This
//! module's only job is to take a discovered
//! [`infinitier_core::save_games::SaveGame`] — which carries the
//! GAM file as a [`DataSource`] — and produce the keeper's parsed
//! [`SaveGame`] (the campaign-wide [`Gam`] plus a pre-parsed party
//! summary so the UI never has to re-walk the CRE blobs).

use infinitier_core::fs::{DataSource, Importer};
use infinitier_core::resource::Engine;
use infinitier_core::resource::gam::{GamImporter, GamNpc};
use infinitier_core::resource::tlk::Tlk;
use infinitier_core::{resource::gam::Gam, save_games::SaveGame as CoreSaveGame};
use infinitier_core::resource::cre::{Cre, CreImporter};

/// A loaded save game — the parsed [`Gam`] plus a parsed per-party
/// summary so the UI can render without re-walking the CRE blobs on
/// every frame.
#[derive(Debug, Clone)]
pub struct SaveGame {
    /// On-disk name of the save folder, copied from
    /// [`infinitier_core::save_games::SaveGame::name`]. Used for
    /// window-title / header-panel display.
    pub name: String,
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

/// Parse the GAM out of `core_save` and decode the party. `engine`
/// drives the engine-specific GAM dispatch; `tlk` (when supplied) is
/// used to resolve each CRE's long-name strref into a human-readable
/// display name. `None` falls back to the GAM's 32-byte localized
/// name slot (only the protagonist is stored there) and then to the
/// 8-byte engine script-name.
pub fn load_save(
    core_save: &CoreSaveGame,
    engine: Engine,
    tlk: Option<&Tlk>,
) -> std::io::Result<SaveGame> {
    let gam_ds = core_save.gam.as_ref().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "save '{}' has no .GAM file next to its .SAV",
                core_save.name
            ),
        )
    })?;
    let gam: Gam = GamImporter {
        name: &core_save.name,
        engine,
    }
    .import(gam_ds)?;
    let party = gam
        .party_npcs
        .iter()
        .enumerate()
        .map(|(idx, npc)| build_party_member(idx, npc, engine, tlk))
        .collect();
    Ok(SaveGame {
        name: core_save.name.clone(),
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
    use infinitier_core::fs::CaseInsensitiveFS;
    use infinitier_core::resource::Game;
use infinitier_core::resource::tlk::TlkImporter;
use infinitier_core::save_games::scan_save_games;

    /// Build a `core::save_games::SaveGame` for the fixture under
    /// `assets/SAV_GAM/<engine_dir>/<save_dir>/<save_name>` using the
    /// same FS-driven discovery the keeper uses at runtime.
    fn fixture_save(engine_dir: &str, save_name: &str) -> CoreSaveGame {
        let fs = CaseInsensitiveFS::new(
            infinitier_test_utils::get_assets_path()
                .join("SAV_GAM")
                .join(engine_dir),
        )
        .expect("opening fixture FS");
        let saves = scan_save_games(&fs);
        saves
            .by_name(save_name)
            .unwrap_or_else(|| panic!("fixture save '{save_name}' not discovered"))
            .clone()
    }

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
        let core_save = fixture_save("bg_ee", "000000000-Auto-Salvataggio");
        let save = load_save(&core_save, Game::Bgee.engine(), None).expect("loading BG:EE save");
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
        let max_strref = entries.iter().map(|(s, _)| *s).max().unwrap_or(0) as usize;
        let n_entries = max_strref + 1;
        let header_len = 0x12usize;
        let entry_len = 26usize;
        let strings_offset = header_len + entry_len * n_entries;
        let mut buf = vec![0u8; strings_offset];
        buf[0..8].copy_from_slice(b"TLK V1  ");
        buf[8..10].copy_from_slice(&1252u16.to_le_bytes());
        buf[10..14].copy_from_slice(&(n_entries as u32).to_le_bytes());
        buf[14..18].copy_from_slice(&(strings_offset as u32).to_le_bytes());
        let mut strings: Vec<u8> = Vec::new();
        for (strref, text) in entries {
            let entry_pos = header_len + (*strref as usize) * entry_len;
            let offset = strings.len() as u32;
            let length = text.len() as u32;
            buf[entry_pos..entry_pos + 2].copy_from_slice(&1u16.to_le_bytes());
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

        let core_save = fixture_save(
            "bg2_ee",
            "000000005-Salvataggio Finale-TOB - Thu Mar 26 21-32-51 2026",
        );
        let engine = Game::Bg2ee.engine();

        // Pass 1: load without TLK so we can read each party slot's
        // CRE long-name strref.
        let no_tlk = load_save(&core_save, engine, None).expect("loading BG2:EE save");
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
            load_save(&core_save, engine, Some(&tlk)).expect("loading BG2:EE save with TLK");
        assert_eq!(
            resolved.party.len(),
            no_tlk.party.len(),
            "party slot count mustn't change when adding a TLK"
        );

        let mut tlk_hits = 0usize;
        for (i, member) in resolved.party.iter().enumerate() {
            if strrefs[i] == u32::MAX {
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
