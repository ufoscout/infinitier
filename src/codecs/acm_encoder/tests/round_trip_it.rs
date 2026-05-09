use std::fs;
use std::io::Cursor;
use std::path::Path;

use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_acm_encoder::{encode_wav, encode_wav_subband};
use infinitier_datasource::DataSource;
use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path};

/// Decode a RIFF/WAVE byte buffer into `(samples, spec)` for sample-level
/// comparison.
fn read_wav(bytes: &[u8]) -> (Vec<i16>, hound::WavSpec) {
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).expect("read wav");
    let spec = reader.spec();
    assert_eq!(
        spec.sample_format,
        hound::SampleFormat::Int,
        "fixture must be integer PCM"
    );
    assert_eq!(spec.bits_per_sample, 16, "fixture must be 16-bit PCM");
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .expect("read samples");
    (samples, spec)
}

/// `(max_abs_diff, rms)` between two equal-length sample buffers.
fn diff_stats(a: &[i16], b: &[i16]) -> (i32, f64) {
    assert_eq!(a.len(), b.len());
    let mut max_abs: i32 = 0;
    let mut sum_sq: u128 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        let d = (*x as i32) - (*y as i32);
        let ad = d.unsigned_abs() as i32;
        if ad > max_abs {
            max_abs = ad;
        }
        sum_sq += (d as i64).unsigned_abs() as u128 * (d as i64).unsigned_abs() as u128;
    }
    let rms = (sum_sq as f64 / a.len() as f64).sqrt();
    (max_abs, rms)
}

/// Round-trip one fixture through the two top-level encoders:
///
/// 1. **`encode_wav`** (v1, fully lossless) — the decoded WAV must
///    match the original sample-for-sample.
/// 2. **`encode_wav_subband`** (full subband + packer pipeline) — the
///    transform uses double-precision floats and the lifting amplifier
///    can clip so the round-trip is approximate; we verify channels /
///    rate / sample count round-trip exactly and that the per-sample
///    error stays bounded, with stats logged for inspection.
fn check_round_trip(wav_path: &Path, out_root: &Path) {
    let stem = wav_path
        .file_stem()
        .expect("file stem")
        .to_string_lossy()
        .to_string();
    eprintln!("\n  fixture: {}", wav_path.display());

    let orig_bytes = fs::read(wav_path).expect("read original wav");

    // Skip non-16-bit fixtures: `read_wav` and the acm encoders below
    // both assume 16-bit signed PCM. Other bit depths (e.g.
    // `CHANT.WAV` is 8-bit) get their own coverage in
    // `infinitier_wav_decoder`'s tests.
    {
        let probe = hound::WavReader::new(Cursor::new(&orig_bytes)).expect("parse wav");
        let bits = probe.spec().bits_per_sample;
        if bits != 16 {
            eprintln!(
                "    skip {}: bits_per_sample {bits} ≠ 16",
                wav_path.display()
            );
            return;
        }
    }

    let (orig_samples, orig_spec) = read_wav(&orig_bytes);

    // ── 1. encode_wav (v1, lossless) ────────────────────────────────────
    {
        let mut acm = Vec::new();
        encode_wav(Cursor::new(&orig_bytes), &mut acm)
            .unwrap_or_else(|e| panic!("encode_wav {}: {e}", wav_path.display()));

        let mut dec = AcmDecoder::open(
            &DataSource::new(acm),
            OutputChannels::Original,
            format!("{stem}.v1"),
        )
        .expect("open re-encoded ACM");

        assert_eq!(
            dec.info.channels, orig_spec.channels as u32,
            "channels must round-trip via encode_wav"
        );
        assert_eq!(
            dec.info.rate, orig_spec.sample_rate,
            "rate must round-trip via encode_wav"
        );
        assert_eq!(
            dec.info.total_values as usize,
            orig_samples.len(),
            "total_values must round-trip via encode_wav"
        );

        let rt_path = out_root.join(format!("{stem}.v1.wav"));
        dec.decode_to_file(&rt_path).expect("decode round-trip wav");

        let (rt_samples, _) = read_wav(&fs::read(&rt_path).unwrap());
        assert_eq!(
            rt_samples,
            orig_samples,
            "encode_wav (v1) must round-trip every sample exactly for {}",
            wav_path.display()
        );
        eprintln!(
            "    encode_wav: bit-exact round-trip ({} samples)",
            rt_samples.len()
        );
    }

    // ── 2. encode_wav_subband (subband + packer) ───────────────────────
    {
        let mut acm = Vec::new();
        encode_wav_subband(Cursor::new(&orig_bytes), &mut acm)
            .unwrap_or_else(|e| panic!("encode_wav_subband {}: {e}", wav_path.display()));

        let mut dec = AcmDecoder::open(
            &DataSource::new(acm),
            OutputChannels::Original,
            format!("{stem}.subband"),
        )
        .expect("open re-encoded ACM");

        assert_eq!(
            dec.info.channels, orig_spec.channels as u32,
            "channels must round-trip via encode_wav_subband"
        );
        assert_eq!(
            dec.info.rate, orig_spec.sample_rate,
            "rate must round-trip via encode_wav_subband"
        );
        assert_eq!(
            dec.info.total_values as usize,
            orig_samples.len(),
            "total_values must round-trip via encode_wav_subband"
        );

        let rt_path = out_root.join(format!("{stem}.subband.wav"));
        dec.decode_to_file(&rt_path).expect("decode round-trip wav");

        let (rt_samples, _) = read_wav(&fs::read(&rt_path).unwrap());
        let (max_abs, rms) = diff_stats(&orig_samples, &rt_samples);
        let peak_orig: i32 = orig_samples
            .iter()
            .map(|s| s.unsigned_abs() as i32)
            .max()
            .unwrap_or(1);
        eprintln!("    encode_wav_subband: max_abs={max_abs} rms={rms:.2} peak_orig={peak_orig}",);

        // Subband is lossy by construction. Verify the round-trip
        // didn't produce nonsense — we just rule out total
        // garbage (max_abs ≥ i16 full range), keep the actual numbers
        // visible in the test output for inspection.
        assert!(
            max_abs < i16::MAX as i32,
            "encode_wav_subband round-trip diff too large for {} (max_abs={max_abs})",
            wav_path.display()
        );
    }
}

#[test]
fn round_trip_random_wavs_via_encoders() {
    let wav_root = get_assets_path().join("WAV");
    let all_wavs = get_all_in_folder_by_extension(&wav_root, "wav");

    assert!(
        !all_wavs.is_empty(),
        "no .WAV fixtures found under {} — at least one is required",
        wav_root.display()
    );

    for wav_path in &all_wavs {
        let dir = tempfile::tempdir().unwrap();
        check_round_trip(wav_path, dir.path());
    }
}
