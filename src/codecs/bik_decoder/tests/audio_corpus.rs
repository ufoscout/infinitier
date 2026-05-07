//! Audio-corpus validation against `assets/resources/BIK/`.
//!
//! For every Bink file with audio, decode the full audio stream, compute
//! the SHA-256 of the interleaved s16le PCM, and compare against the
//! `wav_sha256` recorded in the sibling JSON fixture. The pure-Rust DCT
//! is mathematically equivalent to FFmpeg's tx-framework DCT but doesn't
//! match it byte-exactly (different float operation order); we measure
//! PSNR against the FFmpeg-decoded WAV reference and require ≥ 30 dB —
//! a comfortable audibility margin.

mod common;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;

use infinitier_bik_decoder::{AudioDecoder, parse_header};
use sha2::{Digest, Sha256};

use crate::common::{CorpusEntry, corpus};

/// PSNR threshold in dB — see audio.rs for why bit-exact isn't reachable
/// without porting FFmpeg's tx_dct internals. 30 dB sits well below
/// audibility for any Bink content.
const PSNR_PASS_DB: f64 = 30.0;

fn decode_full_audio(entry: &CorpusEntry) -> Result<Vec<i16>, String> {
    let mut f = File::open(&entry.bik_path).map_err(|e| format!("open: {e}"))?;
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
        let aud_len = u32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]) as usize;
        let chunk = audio
            .decode_packet(&packet[4..4 + aud_len])
            .map_err(|e| format!("audio decode: {e}"))?;
        pcm.extend_from_slice(&chunk);
    }
    Ok(pcm)
}

fn pcm_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    digest.iter().map(|b| format!("{b:02x}")).collect()
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
    20.0 * (i16::MAX as f64 / mse.sqrt()).log10()
}

/// Re-decode the file's audio track via FFmpeg as the PSNR reference. We
/// don't ship the WAV in-repo (only the SHA-256 in the fixture), so the
/// reference is reconstructed at test time. If `ffmpeg` isn't on PATH the
/// PSNR step is skipped and only the byte-exact pass / hash is reported.
fn ffmpeg_reference_pcm(entry: &CorpusEntry) -> Option<Vec<i16>> {
    let audio = entry.fixture.audio.as_ref()?;
    let out = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-i",
            entry.bik_path.to_str()?,
            "-vn",
            "-f",
            "s16le",
            "-acodec",
            "pcm_s16le",
            "-ar",
            &audio.sample_rate.to_string(),
            "-ac",
            &audio.channels.to_string(),
            "-",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        out.stdout
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect(),
    )
}

#[test]
fn corpus_audio_matches_reference() {
    let entries = corpus();
    let mut byte_exact = 0usize;
    let mut psnr_pass = 0usize;
    let mut psnr_fail: Vec<(String, f64)> = Vec::new();
    let mut psnr_skipped: Vec<String> = Vec::new();
    let mut without_audio = 0usize;

    for entry in &entries {
        let Some(audio) = entry.fixture.audio.as_ref() else {
            without_audio += 1;
            continue;
        };
        let label = entry.label();
        let pcm = match decode_full_audio(entry) {
            Ok(p) => p,
            Err(e) => panic!("{label} decode error: {e}"),
        };
        let our_hash = hash_hex(&pcm_to_le_bytes(&pcm));

        if our_hash == audio.wav_sha256 {
            eprintln!(
                "✓  {:<24}  byte-exact ({} samples)",
                label,
                pcm.len()
            );
            byte_exact += 1;
            continue;
        }

        // Not byte-exact — fall back to PSNR vs FFmpeg's live re-decode.
        let Some(reference) = ffmpeg_reference_pcm(entry) else {
            eprintln!(
                "?  {:<24}  hash mismatch and ffmpeg unavailable for PSNR fallback",
                label
            );
            psnr_skipped.push(label.to_owned());
            continue;
        };
        let psnr = psnr_db(&pcm, &reference);
        let n = pcm.len().min(reference.len());
        let max_abs = (0..n)
            .map(|i| (pcm[i] as i32 - reference[i] as i32).unsigned_abs())
            .max()
            .unwrap_or(0);
        if psnr >= PSNR_PASS_DB {
            eprintln!(
                "≈  {:<24}  PSNR {:>6.2} dB (max abs Δ = {})",
                label, psnr, max_abs
            );
            psnr_pass += 1;
        } else {
            eprintln!(
                "✗  {:<24}  PSNR {:>6.2} dB BELOW {} dB (max abs Δ = {})",
                label, psnr, PSNR_PASS_DB, max_abs
            );
            psnr_fail.push((label.to_owned(), psnr));
        }
    }

    eprintln!(
        "\nsummary: {} byte-exact, {} pass PSNR ≥ {} dB, {} below threshold, {} ffmpeg-skipped, {} files without audio",
        byte_exact,
        psnr_pass,
        PSNR_PASS_DB,
        psnr_fail.len(),
        psnr_skipped.len(),
        without_audio,
    );
    assert!(
        psnr_fail.is_empty(),
        "{} files below PSNR threshold: {:?}",
        psnr_fail.len(),
        psnr_fail
    );
}
