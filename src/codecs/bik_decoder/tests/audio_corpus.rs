//! Audio-corpus validation against `assets/BIK/`.
//!
//! For every Bink file with audio, decode the full audio stream, hash
//! the interleaved s16le PCM with SHA-256, and assert it matches the
//! `wav_sha256` recorded in the sibling JSON fixture. Both fixture and
//! test pass through this crate's pure-Rust decoder, so the assertion
//! is a self-consistency / regression check on decoder output.

mod common;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use infinitier_bik_decoder::{AudioDecoder, parse_header};
use sha2::{Digest, Sha256};

use crate::common::{CorpusEntry, corpus};

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
        f.seek(SeekFrom::Start(fr.pos as u64))
            .map_err(|e| format!("seek: {e}"))?;
        f.read_exact(&mut packet)
            .map_err(|e| format!("read: {e}"))?;
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

#[test]
fn corpus_audio_matches_fixture() {
    let entries = corpus();
    let mut byte_exact = 0usize;
    let mut without_audio = 0usize;
    let mut failed: Vec<String> = Vec::new();

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
            eprintln!("✓  {:<24}  byte-exact ({} samples)", label, pcm.len());
            byte_exact += 1;
        } else {
            eprintln!(
                "✗  {:<24}  hash mismatch, got {}.. want {}..",
                label,
                &our_hash[..8],
                &audio.wav_sha256[..8],
            );
            failed.push(label.to_owned());
        }
    }

    eprintln!(
        "\nsummary: {} byte-exact, {} files without audio, {} failed",
        byte_exact,
        without_audio,
        failed.len(),
    );
    assert!(failed.is_empty(), "files with audio mismatch: {:?}", failed);
}
