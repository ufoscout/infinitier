//! Byte-faithful round-trip coverage for save-game resources, over
//! every fixture under `assets/SAV_GAM/`.
//!
//! A keeper save re-serialises the embedded creatures, so the
//! importer→exporter round-trip of every embedded CRE **must** be
//! byte-for-byte identical — otherwise an unmodified save is silently
//! reshaped (the "corrupt on reload" bug). This walks all GAM and SAV
//! fixtures, pulls out every embedded CRE, and asserts exact equality.

use std::path::{Path, PathBuf};

use infinitier_core::fs::{DataSource, Importer};
use infinitier_core::resource::Engine;
use infinitier_core::resource::cre::{CreExporter, CreImporter};
use infinitier_core::resource::gam::GamImporter;
use infinitier_core::resource::sav::SavImporter;
use infinitier_common::Game;
use infinitier_test_utils::get_assets_path;

/// Every file under `assets/SAV_GAM/` with one of `exts` (lowercased).
fn files_with_ext(exts: &[&str]) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&get_assets_path().join("SAV_GAM"), &mut out);
    out.retain(|p| {
        p.extension()
            .and_then(|x| x.to_str())
            .map(|x| exts.iter().any(|e| x.eq_ignore_ascii_case(e)))
            .unwrap_or(false)
    });
    out
}

/// Import a CRE blob and assert it re-exports byte-for-byte.
fn assert_cre_roundtrips(bytes: &[u8], origin: &str) {
    if bytes.len() < 8 || &bytes[0..4] != b"CRE " {
        return; // external resref slot / not an embedded CRE
    }
    let cre = CreImporter {
        name: origin,
        game: Game::Bg2ee,
    }
    .import(&DataSource::new(bytes.to_vec()))
    .unwrap_or_else(|e| panic!("import CRE from {origin}: {e}"));
    let mut out = Vec::new();
    CreExporter
        .export(&cre, &mut out)
        .unwrap_or_else(|e| panic!("export CRE from {origin}: {e}"));
    assert!(
        out == bytes,
        "CRE from {origin} did not round-trip byte-for-byte (orig {} B, export {} B, first diff at {:?})",
        bytes.len(),
        out.len(),
        bytes.iter().zip(&out).position(|(a, b)| a != b),
    );
}

#[test]
fn embedded_cres_in_gam_fixtures_round_trip_byte_for_byte() {
    let gams = files_with_ext(&["gam"]);
    assert!(!gams.is_empty(), "no GAM fixtures found");
    let mut checked = 0usize;
    for path in &gams {
        let label = path.display().to_string();
        let Ok(gam) = (GamImporter {
            name: "gam",
            engine: Engine::Ee,
        })
        .import(&DataSource::new(path.clone())) else {
            continue;
        };
        for (i, npc) in gam
            .party_npcs
            .iter()
            .chain(gam.non_party_npcs.iter())
            .enumerate()
        {
            assert_cre_roundtrips(&npc.cre, &format!("{label}#npc{i}"));
            if npc.cre.len() >= 8 && &npc.cre[0..4] == b"CRE " {
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "expected many embedded CREs, got {checked}");
}

#[test]
fn embedded_cres_in_sav_fixtures_round_trip_byte_for_byte() {
    let savs = files_with_ext(&["sav"]);
    assert!(!savs.is_empty(), "no SAV fixtures found");
    for path in &savs {
        let label = path.display().to_string();
        let Ok(sav) = (SavImporter { name: "sav" }).import(&DataSource::new(path.clone())) else {
            continue;
        };
        for entry in &sav.entries {
            assert_cre_roundtrips(&entry.data, &format!("{label}!{}", entry.filename));
        }
    }
}
