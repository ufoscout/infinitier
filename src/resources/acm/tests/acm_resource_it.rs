use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use infinitier_acm_resource::{Acm, AcmFormat, AcmImporter};
use infinitier_datasource::{DataSource, Importer};
use infinitier_test_utils::get_assets_path;
use sha2::{Digest, Sha256};

/// Pull samples block-by-block until EOF, returning the entire
/// interleaved `i16` buffer.
fn drain_to_pcm(dec: &mut Acm) -> Vec<i16> {
    let total = dec.total_values() as usize;
    let mut samples = Vec::with_capacity(total);
    let mut buf = vec![0i16; 4096];
    loop {
        let n = dec.read_samples(&mut buf).expect("read_samples");
        if n == 0 {
            break;
        }
        samples.extend_from_slice(&buf[..n]);
    }
    samples
}

fn sha256_hex_of_i16_le(samples: &[i16]) -> String {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    let mut h = Sha256::new();
    h.update(&bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Walk one directory tree and return every regular file whose
/// extension is `acm` (case-insensitive).
fn collect_acm_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("read_dir({}): {e}", dir.display()));
        for entry in entries {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            let kind = entry.file_type().expect("file_type");
            if kind.is_dir() {
                stack.push(path);
                continue;
            }
            if kind.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("acm"))
                    .unwrap_or(false)
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Resource name the importer should be given for a particular asset
/// — `bg2/BC1A1.acm` → `BC1A1`. Just for log-label readability; the
/// importer doesn't use it for anything dispatch-affecting.
fn resource_name_for(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("acm")
        .to_string()
}

fn relpath(asset_root: &Path, abs: &Path) -> String {
    abs.strip_prefix(asset_root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn every_asset_acm_imports_cleanly() {
    let root = get_assets_path().join("ACM");
    assert!(root.is_dir(), "missing {}", root.display());

    let acm_dir = root.join("ACM");
    let ogg_dir = root.join("OGG");

    let real_acm = collect_acm_files(&acm_dir);
    let ogg_acm = collect_acm_files(&ogg_dir);

    assert!(!real_acm.is_empty(), "no genuine ACM fixtures found");
    assert!(!ogg_acm.is_empty(), "no OGG-as-ACM fixtures found");

    // Every file in `ACM/ACM/` must come back as `AcmFormat::Acm`.
    for path in &real_acm {
        let rel = relpath(&root, path);
        let name = resource_name_for(path);
        let mut dec = AcmImporter { name: &name }
            .import(&DataSource::new(path.clone()))
            .unwrap_or_else(|e| panic!("[{rel}] import failed: {e}"));
        assert_eq!(
            dec.format(),
            AcmFormat::Acm,
            "[{rel}] expected genuine ACM, got {:?}",
            dec.format(),
        );
        let samples = drain_to_pcm(&mut dec);
        assert!(
            !samples.is_empty(),
            "[{rel}] decoder produced zero samples",
        );
        // Sanity: drained count must match the declared total or be
        // off only by a small EOF-rounding amount (per the existing
        // ACM/WAV conventions). Allow ±channels samples of slack.
        let expected = dec.total_values() as usize;
        let slack = dec.channels() as usize;
        assert!(
            samples.len().abs_diff(expected) <= slack,
            "[{rel}] drained {} != declared {} (slack {})",
            samples.len(),
            expected,
            slack,
        );
    }

    // Every file in `ACM/OGG/` must come back as `AcmFormat::Ogg`.
    for path in &ogg_acm {
        let rel = relpath(&root, path);
        let name = resource_name_for(path);
        let mut dec = AcmImporter { name: &name }
            .import(&DataSource::new(path.clone()))
            .unwrap_or_else(|e| panic!("[{rel}] import failed: {e}"));
        assert_eq!(
            dec.format(),
            AcmFormat::Ogg,
            "[{rel}] expected OGG-as-ACM, got {:?}",
            dec.format(),
        );
        let samples = drain_to_pcm(&mut dec);
        assert!(
            !samples.is_empty(),
            "[{rel}] decoder produced zero samples",
        );
    }
}

/// Regression fixtures for the OGG-as-ACM branch. Each entry is
/// `(path relative to assets/ACM, expected SHA-256 of interleaved
/// little-endian i16 PCM)`. The hash is the byte-exact equivalent of
/// the `data` section of the WAV file the OGG would render into via
/// `WavDecoder::decode_to_file`.
///
/// To regenerate (e.g. after a Symphonia upgrade), run:
///
/// ```sh
/// INFINITIER_RECORD_ACM_OGG_HASHES=1 \
///   cargo test -p infinitier_acm_resource \
///   --test acm_resource_it ogg_acm_assets_decode_to_baseline_hash \
///   -- --nocapture
/// ```
///
/// then paste the printed lines back here.
const OGG_BASELINE_HASHES: &[(&str, &str)] = &[
    (
        "OGG/bgee/bc1a1.acm",
        "5e25d74482c75f1dbd85e5c3ef3c8a53ed06e61b01ecdf6891da671a3f47c9a5",
    ),
];

#[test]
fn ogg_acm_assets_decode_to_baseline_hash() {
    let root = get_assets_path().join("ACM");
    let record_mode = std::env::var("INFINITIER_RECORD_ACM_OGG_HASHES").is_ok();

    // Re-derive the list of OGG-as-ACM assets so any new file added
    // to `assets/ACM/OGG/` is flagged as missing from the baseline.
    let ogg_files = collect_acm_files(&root.join("OGG"));
    let actual_rels: Vec<String> = ogg_files
        .iter()
        .map(|p| relpath(&root, p))
        .collect();
    let baseline_rels: Vec<&str> = OGG_BASELINE_HASHES.iter().map(|(p, _)| *p).collect();
    if !record_mode {
        // Fail loudly if a fixture is added without a paired baseline.
        for got in &actual_rels {
            assert!(
                baseline_rels.contains(&got.as_str()),
                "no baseline for {got}; \
                 add the hash to OGG_BASELINE_HASHES or run with \
                 INFINITIER_RECORD_ACM_OGG_HASHES=1 to print them",
            );
        }
        for want in &baseline_rels {
            assert!(
                actual_rels.contains(&want.to_string()),
                "baseline references {want} but the file is missing",
            );
        }
    }

    let mut recorded: Vec<(String, String)> = Vec::new();
    for path in &ogg_files {
        let rel = relpath(&root, path);
        let name = resource_name_for(path);
        let _ = BufReader::new(File::open(path).expect("open .acm"));
        let mut dec = AcmImporter { name: &name }
            .import(&DataSource::new(path.clone()))
            .unwrap_or_else(|e| panic!("[{rel}] import: {e}"));
        assert_eq!(dec.format(), AcmFormat::Ogg, "[{rel}] expected OGG");
        let samples = drain_to_pcm(&mut dec);
        let hash = sha256_hex_of_i16_le(&samples);

        if record_mode {
            eprintln!("    (\"{rel}\", \"{hash}\"),");
            recorded.push((rel, hash));
            continue;
        }

        let expected = OGG_BASELINE_HASHES
            .iter()
            .find(|(p, _)| *p == rel)
            .map(|(_, h)| *h)
            .expect("baseline entry present (checked above)");
        assert_eq!(
            hash, expected,
            "[{rel}] decoded PCM hash diverged from baseline",
        );
    }

    if record_mode {
        eprintln!(
            "\nRecorded {} OGG-as-ACM hashes; paste them into OGG_BASELINE_HASHES.",
            recorded.len(),
        );
    }
}
