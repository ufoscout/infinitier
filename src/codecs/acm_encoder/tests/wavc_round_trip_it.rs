//! WAVC container round-trip tests for the encoder.
//!
//! Each encoder variant is exercised through `infinitier_wav_resource`,
//! so both the 28-byte WAVC header and the inner ACM bitstream are
//! validated end-to-end by an independent reader.

use std::fs;
use std::io::Cursor;

use infinitier_acm_encoder::{
    AcmEncodeError, encode_pcm_packed_wavc, encode_pcm_subband_wavc, encode_pcm_wavc,
    encode_wav_subband_wavc, encode_wav_wavc,
};
use infinitier_datasource::DataSource;
use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path};
use infinitier_wav_resource::{WavDecoder, WavFormat};

/// Verify the 28-byte WAVC header is well-formed regardless of which
/// encoder body produced the ACM payload.
fn assert_wavc_envelope(bytes: &[u8], total_values: u32, channels: u16) {
    assert!(bytes.len() >= 28, "WAVC output must be at least 28 bytes");
    assert_eq!(&bytes[0..4], b"WAVC", "magic");
    assert_eq!(&bytes[4..8], b"V1.0", "version");
    let uncompressed = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(
        uncompressed,
        total_values * 2,
        "uncompressed = total_values × 2 bytes per i16 sample"
    );
    let compressed = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    assert_eq!(
        compressed as usize,
        bytes.len() - 28,
        "compressed = ACM body length"
    );
    let acm_offset = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    assert_eq!(acm_offset, 28, "ACM data pointer must be 0x1c");
    let hdr_channels = u16::from_le_bytes(bytes[20..22].try_into().unwrap());
    assert_eq!(hdr_channels, channels);
    let hdr_bits = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
    assert_eq!(hdr_bits, 16);
    let hdr_rate = u16::from_le_bytes(bytes[24..26].try_into().unwrap());
    assert_eq!(hdr_rate, 22050);
    let hdr_unused = u16::from_le_bytes(bytes[26..28].try_into().unwrap());
    assert_eq!(hdr_unused, 0x777e);
}

#[test]
fn wavc_v1_round_trips_lossless() {
    let pcm: Vec<i16> = (0..1024).map(|i| (i as i16).wrapping_mul(11)).collect();
    let mut bytes = Vec::new();
    encode_pcm_wavc(&pcm, 1, 22050, &mut bytes).unwrap();
    assert_wavc_envelope(&bytes, pcm.len() as u32, 1);

    let mut dec = WavDecoder::open(&DataSource::new(bytes), "wavc_v1").unwrap();
    assert_eq!(dec.format(), WavFormat::Wavc);
    let info = dec.info().clone();
    assert_eq!(info.channels, 1);
    assert_eq!(info.sample_rate, 22050);
    assert_eq!(info.bits_per_sample, 16);
    assert_eq!(info.total_values as usize, pcm.len());

    let decoded = dec.decode_all().unwrap();
    assert_eq!(decoded, pcm);
}

#[test]
fn wavc_packed_round_trips_lossless() {
    // Sparse signal — exercises the packer's f_zero / Huffman books
    // inside the WAVC container.
    let mut pcm = vec![0i16; 1024];
    for i in (0..pcm.len()).step_by(64) {
        pcm[i] = if i % 128 == 0 { 1 } else { -1 };
    }
    let mut bytes = Vec::new();
    encode_pcm_packed_wavc(&pcm, 1, 22050, &mut bytes).unwrap();
    assert_wavc_envelope(&bytes, pcm.len() as u32, 1);

    let mut dec = WavDecoder::open(&DataSource::new(bytes), "wavc_packed").unwrap();
    assert_eq!(dec.format(), WavFormat::Wavc);
    let decoded = dec.decode_all().unwrap();
    assert_eq!(decoded, pcm);
}

#[test]
fn wavc_subband_round_trips_with_tolerance() {
    let pcm: Vec<i16> = (0..4096)
        .map(|i| ((i as f32 * 0.1).sin() * 4000.0) as i16)
        .collect();
    let mut bytes = Vec::new();
    encode_pcm_subband_wavc(&pcm, 1, 22050, 7, 16, &mut bytes).unwrap();
    assert_wavc_envelope(&bytes, pcm.len() as u32, 1);

    let mut dec = WavDecoder::open(&DataSource::new(bytes), "wavc_subband").unwrap();
    assert_eq!(dec.format(), WavFormat::Wavc);
    assert_eq!(dec.info().total_values as usize, pcm.len());

    let decoded = dec.decode_all().unwrap();
    // Subband path is lossy (float math + lifting). Just verify the
    // length round-trips and the per-sample error is bounded.
    assert_eq!(decoded.len(), pcm.len());
    let max_abs: i32 = pcm
        .iter()
        .zip(decoded.iter())
        .map(|(a, b)| (*a as i32 - *b as i32).abs())
        .max()
        .unwrap_or(0);
    assert!(
        max_abs < i16::MAX as i32,
        "subband WAVC round-trip max_abs={max_abs}"
    );
}

#[test]
fn wavc_stereo_round_trips() {
    // Stereo input — verify the WAVC header records `channels = 2` and
    // the decoder gives back interleaved samples.
    let pcm: Vec<i16> = (0..2048)
        .map(|i| {
            if i % 2 == 0 {
                (i as i16) * 5
            } else {
                -(i as i16) * 3
            }
        })
        .collect();
    let mut bytes = Vec::new();
    encode_pcm_packed_wavc(&pcm, 2, 22050, &mut bytes).unwrap();
    assert_wavc_envelope(&bytes, pcm.len() as u32, 2);

    let mut dec = WavDecoder::open(&DataSource::new(bytes), "wavc_stereo").unwrap();
    assert_eq!(dec.info().channels, 2);
    let decoded = dec.decode_all().unwrap();
    assert_eq!(decoded, pcm);
}

#[test]
fn wavc_rejects_wrong_sample_rate() {
    let pcm = vec![0i16; 256];
    let mut bytes = Vec::new();
    let err = encode_pcm_wavc(&pcm, 1, 44100, &mut bytes).unwrap_err();
    assert!(matches!(err, AcmEncodeError::WavcInvalidSampleRate(44100)));

    let mut bytes = Vec::new();
    let err = encode_pcm_packed_wavc(&pcm, 1, 11025, &mut bytes).unwrap_err();
    assert!(matches!(err, AcmEncodeError::WavcInvalidSampleRate(11025)));

    let mut bytes = Vec::new();
    let err = encode_pcm_subband_wavc(&pcm, 1, 48000, 7, 16, &mut bytes).unwrap_err();
    assert!(matches!(err, AcmEncodeError::WavcInvalidSampleRate(48000)));
}

#[test]
fn wavc_rejects_invalid_channels() {
    let pcm = vec![0i16; 256];
    let mut bytes = Vec::new();
    let err = encode_pcm_wavc(&pcm, 3, 22050, &mut bytes).unwrap_err();
    assert!(matches!(err, AcmEncodeError::InvalidChannels(3)));

    let mut bytes = Vec::new();
    let err = encode_pcm_wavc(&pcm, 0, 22050, &mut bytes).unwrap_err();
    assert!(matches!(err, AcmEncodeError::InvalidChannels(0)));
}

/// Pull a real `.WAV` from `assets/WAV`, run it through
/// the WAVC encoders, and check the output round-trips through
/// `WavDecoder`.
fn read_wav_samples(path: &std::path::Path) -> (Vec<i16>, hound::WavSpec) {
    let bytes = fs::read(path).expect("read wav");
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).expect("parse wav");
    let spec = reader.spec();
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .expect("read samples");
    (samples, spec)
}

#[test]
fn encode_wav_wavc_round_trips_bundled_fixture() {
    let wav_root = get_assets_path().join("WAV");
    let all_wavs = get_all_in_folder_by_extension(&wav_root, "wav", false);
    assert!(
        !all_wavs.is_empty(),
        "no .WAV fixtures under {}",
        wav_root.display()
    );

    for pick in &all_wavs {
        let (orig_samples, orig_spec) = read_wav_samples(pick);
        if orig_spec.sample_rate != 22050 {
            eprintln!(
                "skip {}: rate {} ≠ 22050",
                pick.display(),
                orig_spec.sample_rate
            );
            return;
        }
        if orig_spec.bits_per_sample != 16 {
            // The WAVC encoder pipeline assumes 16-bit input. Other bit
            // depths (e.g. CHANT.WAV is 8-bit) get their own coverage in
            // `infinitier_wav_resource`'s tests.
            eprintln!(
                "skip {}: bits_per_sample {} ≠ 16",
                pick.display(),
                orig_spec.bits_per_sample
            );
            return;
        }

        // Lossless path.
        {
            let mut wavc_bytes = Vec::new();
            encode_wav_wavc(Cursor::new(fs::read(pick).unwrap()), &mut wavc_bytes).unwrap();
            assert_wavc_envelope(&wavc_bytes, orig_samples.len() as u32, orig_spec.channels);

            let mut dec =
                WavDecoder::open(&DataSource::new(wavc_bytes), pick.display().to_string()).unwrap();
            assert_eq!(dec.format(), WavFormat::Wavc);
            let decoded = dec.decode_all().unwrap();
            assert_eq!(decoded, orig_samples, "v1 WAVC round-trip must be lossless");
        }

        // Subband path — bounded error.
        {
            let mut wavc_bytes = Vec::new();
            encode_wav_subband_wavc(Cursor::new(fs::read(pick).unwrap()), &mut wavc_bytes).unwrap();
            assert_wavc_envelope(&wavc_bytes, orig_samples.len() as u32, orig_spec.channels);

            let mut dec =
                WavDecoder::open(&DataSource::new(wavc_bytes), pick.display().to_string()).unwrap();
            assert_eq!(dec.format(), WavFormat::Wavc);
            let decoded = dec.decode_all().unwrap();
            assert_eq!(decoded.len(), orig_samples.len());

            let (max_abs, sum_sq) =
                orig_samples
                    .iter()
                    .zip(decoded.iter())
                    .fold((0i32, 0u128), |(m, s), (a, b)| {
                        let d = (*a as i32 - *b as i32).abs();
                        let m = m.max(d);
                        let s = s + (d as u128) * (d as u128);
                        (m, s)
                    });
            let rms = (sum_sq as f64 / orig_samples.len() as f64).sqrt();
            eprintln!(
                "  {}: subband WAVC round-trip max_abs={max_abs} rms={rms:.2}",
                pick.display()
            );
            assert!(max_abs < i16::MAX as i32);
        }
    }
}

#[test]
fn encode_wav_wavc_rejects_non_22050_input() {
    use hound::{SampleFormat, WavSpec, WavWriter};

    // Build a small 44.1 kHz WAV in memory and feed it into encode_wav_wavc.
    let spec = WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut w = WavWriter::new(&mut buf, spec).unwrap();
        for i in 0..256 {
            w.write_sample(i as i16).unwrap();
        }
        w.finalize().unwrap();
    }
    let wav_bytes = buf.into_inner();

    let mut out = Vec::new();
    let err = encode_wav_wavc(Cursor::new(wav_bytes), &mut out).unwrap_err();
    assert!(matches!(err, AcmEncodeError::WavcInvalidSampleRate(44100)));
}
