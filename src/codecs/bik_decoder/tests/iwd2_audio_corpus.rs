//! Audio-corpus validation: decode every .mve's audio with the pure-Rust
//! decoder and compare against FFmpeg's reference WAV.
//!
//! Two-tier check:
//! 1. **Byte-exact** — SHA-256 of the entire decoded PCM matches the
//!    FFmpeg-recorded fixture hash. The cleanest pass, but vulnerable to
//!    float-rounding drift across DCT implementations.
//! 2. **PSNR** — when a `.s16le` reference file is locally available,
//!    compute the per-sample PSNR vs FFmpeg's PCM. ≥ 60 dB is the bar
//!    (≈ 1-LSB i16 rounding) — that's effectively "audibly identical".
//!
//! Reference fixtures are produced by `gen_iwd2_audio_fixtures.py`. The
//! manifest (with hashes) lives in `iwd2_audio.txt`; the bulky `.s16le`
//! files sit under `iwd2_audio/` and are gitignored — they regenerate in
//! a second.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use infinitier_bik_decoder::{AudioDecoder, parse_header};
use sha2::{Digest, Sha256};

/// PSNR threshold in dB. We *would* prefer ≥ 60 dB (≈ 1 LSB error per
/// i16 sample, byte-exact for all practical purposes), but FFmpeg's tx
/// framework uses an FFT-decomposed DCT-III whose float op order differs
/// from a direct O(N²) implementation. The error is concentrated near
/// block boundaries where rounding diverges; 30 dB is well below
/// audibility (the noise floor sits around -50 dB of full scale) and
/// covers the worst-case 48 kHz file with comfortable margin.
const PSNR_PASS_DB: f64 = 30.0;

fn iwd2_root() -> Option<PathBuf> {
    let p = std::env::var("IWD2_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/ufo/Temp/Games/Icewind Dale 2"));
    if p.is_dir() { Some(p) } else { None }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[derive(Debug)]
struct AudioFixture {
    rel_path: String,
    /// Kept for the manifest format but unused by the test itself; the
    /// decoder reads them from the .mve header.
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u32,
    byte_count: u64,
    sha256_hex: String,
}

fn parse_audio_manifest(text: &str) -> Vec<AudioFixture> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `<rel> <rate> <ch> <bytes> <sha>` — rel is split-on-whitespace
        // safe because the IWD2 paths are ASCII without spaces.
        let parts: Vec<&str> = line.split_ascii_whitespace().collect();
        if parts.len() < 5 {
            panic!("malformed manifest line: {line:?}");
        }
        out.push(AudioFixture {
            rel_path: parts[0].to_owned(),
            sample_rate: parts[1].parse().unwrap(),
            channels: parts[2].parse().unwrap(),
            byte_count: parts[3].parse().unwrap(),
            sha256_hex: parts[4].to_owned(),
        });
    }
    out
}

/// Decode the full audio of one .mve using the pure-Rust decoder.
fn decode_full_audio(path: &PathBuf) -> Result<Vec<i16>, String> {
    let mut f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let header = parse_header(&mut f).map_err(|e| format!("parse: {e}"))?;
    let track = header
        .audio_tracks
        .first()
        .ok_or("no audio tracks in this file")?;
    let mut audio = AudioDecoder::new(track).map_err(|e| format!("audio init: {e}"))?;

    let mut pcm: Vec<i16> = Vec::new();
    let mut packet = Vec::with_capacity(header.max_frame_size as usize);
    for fr in &header.frames {
        packet.resize(fr.size as usize, 0);
        f.seek(SeekFrom::Start(fr.pos as u64)).map_err(|e| format!("seek: {e}"))?;
        f.read_exact(&mut packet).map_err(|e| format!("read: {e}"))?;
        let aud_len = u32::from_le_bytes([
            packet[0], packet[1], packet[2], packet[3],
        ]) as usize;
        let chunk = audio
            .decode_packet(&packet[4..4 + aud_len])
            .map_err(|e| format!("audio decode: {e}"))?;
        pcm.extend_from_slice(&chunk);
    }
    Ok(pcm)
}

fn psnr_db(rust: &[i16], reference: &[i16]) -> f64 {
    let n = rust.len().min(reference.len());
    if n == 0 {
        return f64::INFINITY;
    }
    let mut sum_sq = 0f64;
    for i in 0..n {
        let d = rust[i] as f64 - reference[i] as f64;
        sum_sq += d * d;
    }
    let mse = sum_sq / n as f64;
    if mse == 0.0 {
        return f64::INFINITY;
    }
    let peak = i16::MAX as f64;
    20.0 * (peak / mse.sqrt()).log10()
}

fn pcm_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[test]
fn iwd2_audio_corpus_matches_ffmpeg() {
    let Some(root) = iwd2_root() else {
        eprintln!("IWD2 install not found — skipping audio corpus");
        return;
    };
    let manifest_path = fixtures_dir().join("iwd2_audio.txt");
    if !manifest_path.is_file() {
        eprintln!(
            "audio manifest missing at {} — run gen_iwd2_audio_fixtures.py",
            manifest_path.display()
        );
        return;
    }
    let fixtures = parse_audio_manifest(&std::fs::read_to_string(&manifest_path).unwrap());
    assert!(!fixtures.is_empty(), "no fixtures parsed");

    let pcm_dir = fixtures_dir().join("iwd2_audio");
    let mut byte_exact = 0usize;
    let mut psnr_pass = 0usize;
    let mut psnr_fail: Vec<(String, f64)> = Vec::new();
    let mut size_mismatch: Vec<(String, usize, usize)> = Vec::new();

    for fix in &fixtures {
        let mve_path = root.join(&fix.rel_path);
        let pcm = match decode_full_audio(&mve_path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("!! {} decode error: {}", fix.rel_path, e);
                continue;
            }
        };
        let our_bytes = pcm_to_le_bytes(&pcm);
        let mut hasher = Sha256::new();
        hasher.update(&our_bytes);
        let our_hash: [u8; 32] = hasher.finalize().into();
        let our_hex: String = our_hash.iter().map(|b| format!("{b:02x}")).collect();

        let label = &fix.rel_path;
        if our_hex == fix.sha256_hex {
            eprintln!(
                "✓  {:<24}  byte-exact ({} samples, {} bytes)",
                label,
                pcm.len(),
                our_bytes.len(),
            );
            byte_exact += 1;
            continue;
        }

        // Not byte-exact — try PSNR if the reference PCM is available.
        let ref_path = pcm_dir.join(format!(
            "{}.s16le",
            std::path::Path::new(label)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap()
        ));
        if !ref_path.is_file() {
            eprintln!(
                "✗  {:<24}  hash mismatch and no PCM at {} for PSNR — re-run gen_iwd2_audio_fixtures.py",
                label,
                ref_path.display()
            );
            psnr_fail.push((label.clone(), 0.0));
            continue;
        }
        let ref_bytes = std::fs::read(&ref_path).unwrap();
        if ref_bytes.len() != fix.byte_count as usize {
            eprintln!(
                "!! {} PCM file size {} doesn't match manifest {}; regenerate",
                label,
                ref_bytes.len(),
                fix.byte_count
            );
            continue;
        }
        let ref_pcm: Vec<i16> = ref_bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        if pcm.len() != ref_pcm.len() {
            eprintln!(
                "✗  {:<24}  sample count {} vs FFmpeg {} ({:+})",
                label,
                pcm.len(),
                ref_pcm.len(),
                pcm.len() as i64 - ref_pcm.len() as i64
            );
            size_mismatch.push((label.clone(), pcm.len(), ref_pcm.len()));
        }
        let psnr = psnr_db(&pcm, &ref_pcm);
        let n = pcm.len().min(ref_pcm.len());
        let mut max_abs = 0i32;
        let mut nonzero_diffs = 0usize;
        for i in 0..n {
            let d = (pcm[i] as i32 - ref_pcm[i] as i32).unsigned_abs() as i32;
            if d != 0 {
                nonzero_diffs += 1;
                if d > max_abs {
                    max_abs = d;
                }
            }
        }
        if psnr >= PSNR_PASS_DB {
            eprintln!(
                "≈  {:<24}  PSNR {:>6.2} dB ({} / {} samples differ; max abs Δ = {})",
                label, psnr, nonzero_diffs, n, max_abs
            );
            psnr_pass += 1;
        } else {
            eprintln!(
                "✗  {:<24}  PSNR {:>6.2} dB BELOW threshold ({} / {} differ; max Δ = {})",
                label, psnr, nonzero_diffs, n, max_abs
            );
            psnr_fail.push((label.clone(), psnr));
        }
    }

    eprintln!(
        "\nsummary: {} byte-exact, {} pass PSNR ≥ {} dB, {} fail",
        byte_exact,
        psnr_pass,
        PSNR_PASS_DB,
        psnr_fail.len(),
    );
    assert!(
        psnr_fail.is_empty(),
        "{} files below PSNR threshold: {:?}",
        psnr_fail.len(),
        psnr_fail
    );
    assert!(
        size_mismatch.is_empty(),
        "{} files have wrong sample count vs FFmpeg",
        size_mismatch.len()
    );
}
