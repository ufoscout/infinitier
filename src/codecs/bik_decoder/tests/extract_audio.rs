//! End-to-end check of [`extract_audio_to_wav`].
//!
//! For every corpus entry:
//!
//! * Files **without** audio still produce a WAV at the destination
//!   path (matching `MveDecoder::extract_audio_to_wav`'s contract).
//! * Files **with** audio produce a WAV whose interleaved s16le PCM
//!   matches the `wav_sha256` recorded in the JSON fixture (which is
//!   itself the SHA-256 of the same Rust decoder's `decode_packet`
//!   output — so this test verifies the WAV writer doesn't drop or
//!   reorder any samples).

mod common;

use infinitier_bik_decoder::extract_audio_to_wav;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

use crate::common::corpus;

fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_wav_samples(path: &std::path::Path) -> (hound::WavSpec, Vec<i16>) {
    let mut reader = hound::WavReader::open(path).expect("open wav");
    let spec = reader.spec();
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .expect("read i16 samples");
    (spec, samples)
}

fn pcm_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[test]
fn extract_audio_matches_fixture_pcm() {
    let entries = corpus();
    let dir = tempdir().expect("tempdir");

    let mut with_audio = 0usize;
    let mut without_audio = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for entry in &entries {
        let label = entry.label();
        let dest = dir.path().join(format!("{label}.wav"));
        extract_audio_to_wav(&entry.bik_path, &dest)
            .unwrap_or_else(|e| panic!("{label} extract: {e}"));
        assert!(dest.is_file(), "{label}: WAV not created at {}", dest.display());

        let (spec, samples) = read_wav_samples(&dest);

        match entry.fixture.audio.as_ref() {
            Some(audio) => {
                assert_eq!(spec.channels as u32, audio.channels, "{label}: channels");
                assert_eq!(spec.sample_rate, audio.sample_rate, "{label}: sample_rate");
                assert_eq!(spec.bits_per_sample, 16, "{label}: bits_per_sample");
                assert_eq!(
                    samples.len() as u64,
                    audio.total_samples,
                    "{label}: total_samples",
                );
                let got = hash_hex(&pcm_to_le_bytes(&samples));
                if got == audio.wav_sha256 {
                    eprintln!("✓  {:<24}  {} samples, byte-exact", label, samples.len());
                    with_audio += 1;
                } else {
                    eprintln!(
                        "✗  {:<24}  hash mismatch: got {}.. want {}..",
                        label,
                        &got[..8],
                        &audio.wav_sha256[..8],
                    );
                    failed.push(label.to_owned());
                }
            }
            None => {
                // No-audio inputs must still produce a header-only WAV.
                assert!(
                    samples.is_empty(),
                    "{label}: expected empty WAV but got {} samples",
                    samples.len(),
                );
                eprintln!("✓  {:<24}  no audio → empty WAV", label);
                without_audio += 1;
            }
        }
    }

    eprintln!(
        "\nsummary: {} with audio, {} without audio, {} failed",
        with_audio,
        without_audio,
        failed.len(),
    );
    assert!(failed.is_empty(), "files mismatched: {failed:?}");
}
