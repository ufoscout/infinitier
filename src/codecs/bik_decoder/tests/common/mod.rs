//! Shared helpers for the asset-corpus tests.
//!
//! All Bink fixtures live under `assets/BIK/` next to a
//! `<stem>.json` manifest produced by this crate's `gen_fixtures`
//! example (run via `cargo run --example gen_fixtures --release`). The
//! schema is a near-clone of the MVE fixture at
//! `assets/MVE/16_bits/BISLOGO.json`, with `codec_tag`
//! replacing the MVE-only `palette_bits` field.

#![allow(dead_code)] // each consumer test pulls a different subset.

use std::path::{Path, PathBuf};

use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path, parse_json_file};
use serde::{Deserialize, Serialize};

/// Folder name under `assets/` that holds the Bink corpus.
const BIK_FOLDER: &str = "BIK";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorpusFixture {
    pub video: VideoFixture,
    pub audio: Option<AudioFixture>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoFixture {
    /// 4-byte codec tag taken from the file signature, e.g. `"BIKi"`,
    /// `"BIKb"`, `"BIKf"`. Recorded for diagnostics.
    pub codec_tag: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub frame_duration_us: u64,
    pub frame_hashes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioFixture {
    pub channels: u32,
    pub sample_rate: u32,
    pub bits_per_sample: u32,
    pub format: String,
    pub total_samples: u64,
    pub wav_sha256: String,
}

/// One discovered asset: the path to the binary file and its parsed
/// fixture.
pub struct CorpusEntry {
    pub bik_path: PathBuf,
    pub fixture: CorpusFixture,
}

impl CorpusEntry {
    pub fn label(&self) -> &str {
        self.bik_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
    }

    /// Every codec tag in the asset corpus is decodable now (BinkB landed
    /// in #38, BIKk hooks in #39). Kept as a method so future variants
    /// can opt out cleanly without re-shaping every test loop.
    pub fn is_decodable_video(&self) -> bool {
        true
    }
}

/// Walk `assets/BIK/` and return one entry per Bink file. Pairs
/// each binary with its sibling `<stem>.json`. Recognised extensions:
/// `.bik` and `.mve` (case-insensitive).
pub fn corpus() -> Vec<CorpusEntry> {
    let folder = get_assets_path().join(BIK_FOLDER);
    assert!(folder.is_dir(), "missing folder {}", folder.display());

    let mut paths: Vec<PathBuf> = Vec::new();
    paths.extend(get_all_in_folder_by_extension(&folder, "bik"));
    paths.extend(get_all_in_folder_by_extension(&folder, "mve"));
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no .bik/.mve files in {}",
        folder.display()
    );

    paths
        .into_iter()
        .map(|p| {
            let json_path = json_for(&p);
            assert!(
                json_path.is_file(),
                "missing fixture {} (run `cargo run -p infinitier_bik_decoder --example gen_fixtures --release` to regenerate)",
                json_path.display()
            );
            let fixture: CorpusFixture = parse_json_file(&json_path);
            CorpusEntry { bik_path: p, fixture }
        })
        .collect()
}

/// Returns `<stem>.json` next to a Bink file. Mirrors the convention used
/// by the MVE fixtures (`BISLOGO.MVE` → `BISLOGO.json`).
pub fn json_for(bik_path: &Path) -> PathBuf {
    bik_path.with_extension("json")
}
