use std::io::Cursor;

use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_acm_encoder::{
    encode_pcm, encode_pcm_packed, encode_pcm_packed_with_block_size, encode_pcm_with_block_size,
    encode_wav,
};
use infinitier_datasource::DataSource;

fn round_trip(samples: &[i16], channels: u32, sample_rate: u32) -> Vec<i16> {
    let mut buf = Vec::new();
    encode_pcm(samples, channels, sample_rate, &mut buf).expect("encode failed");
    let dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "round_trip",
    )
    .expect("open failed");
    assert_eq!(dec.info.channels, channels, "channel count must round-trip");
    assert_eq!(
        dec.info.rate, sample_rate,
        "sample rate must round-trip"
    );
    assert_eq!(
        dec.info.total_values as usize,
        samples.len(),
        "total_values must round-trip"
    );
    let mut dec = dec;
    dec.decode_all().expect("decode failed")
}

#[test]
fn round_trip_mono_short() {
    let pcm: Vec<i16> = vec![0, 1, -1, 12345, -12345, 32767, -32768, 100, 200, 300];
    let out = round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_mono_one_block_exact() {
    // Exactly one default-sized block (512 samples).
    let pcm: Vec<i16> = (0..512)
        .map(|i| (i as i16).wrapping_mul(37))
        .collect();
    let out = round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_mono_partial_last_block() {
    // 512 + 17 samples — last block is partial; the decoder must stop
    // at total_values, ignoring the encoder's zero padding.
    let pcm: Vec<i16> = (0..529)
        .map(|i| ((i * 91) as i16).wrapping_sub((i * 7) as i16))
        .collect();
    let out = round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_stereo() {
    // Stereo, multiple full blocks.
    let pcm: Vec<i16> = (0..2048)
        .map(|i| {
            if i % 2 == 0 {
                ((i / 2) as i16).wrapping_mul(13)
            } else {
                -(((i / 2) as i16).wrapping_mul(13))
            }
        })
        .collect();
    let out = round_trip(&pcm, 2, 44100);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_extreme_values() {
    // Edge values: i16::MIN, i16::MAX, 0, ±1 — exercises the b±middle
    // boundaries.
    let pcm = vec![
        i16::MIN,
        i16::MIN + 1,
        -1,
        0,
        1,
        i16::MAX - 1,
        i16::MAX,
        // Repeat to span >1 block.
        i16::MIN,
        i16::MAX,
        0,
    ];
    let mut padded = Vec::new();
    for _ in 0..200 {
        padded.extend_from_slice(&pcm);
    }
    let out = round_trip(&padded, 1, 22050);
    assert_eq!(out, padded);
}

#[test]
fn round_trip_small_block_size() {
    let pcm: Vec<i16> = (0..1000)
        .map(|i| (i as i16).wrapping_mul(11))
        .collect();
    let mut buf = Vec::new();
    encode_pcm_with_block_size(&pcm, 1, 22050, 8, &mut buf).unwrap();
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "small_block",
    )
    .unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out, pcm);
}

/// Encode via the packer, decode via AcmDecoder, return the decoded
/// samples for assertion in tests.
fn packer_round_trip(samples: &[i16], channels: u32, sample_rate: u32) -> Vec<i16> {
    let mut buf = Vec::new();
    encode_pcm_packed(samples, channels, sample_rate, &mut buf).expect("encode failed");
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "packer_round_trip",
    )
    .expect("open failed");
    assert_eq!(
        dec.info.channels, channels,
        "channel count must round-trip"
    );
    assert_eq!(dec.info.rate, sample_rate, "sample rate must round-trip");
    assert_eq!(
        dec.info.total_values as usize,
        samples.len(),
        "total_values must round-trip"
    );
    dec.decode_all().expect("decode failed")
}

#[test]
fn packer_round_trip_silence() {
    // All-zero input — every column should pack as f_zero (ind=0).
    let pcm = vec![0i16; 256];
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_small_amplitude_picks_huffman_books() {
    // Values in {-1, 0, 1} — pack_column should pick K12/K13/T15.
    let pcm: Vec<i16> = (0..512).map(|i| ((i % 3) as i16) - 1).collect();
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_full_i16_range() {
    // Wide-amplitude signal — falls into the default linear branch.
    let pcm: Vec<i16> = (0..1024)
        .map(|i| ((i as f32 * 0.1).sin() * 32000.0) as i16)
        .collect();
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_extreme_values() {
    // Edge values exercise the b±middle boundaries of make_linear at
    // bits=16, plus stress the granulator's pwr derivation.
    let mut pcm = Vec::new();
    for _ in 0..200 {
        pcm.extend_from_slice(&[i16::MIN, i16::MIN + 1, -1, 0, 1, i16::MAX - 1, i16::MAX]);
    }
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_partial_last_block() {
    // Block size doesn't divide len → trailing partial block padded
    // with zeros; the decoder must stop at the encoded total_values.
    let pcm: Vec<i16> = (0..529)
        .map(|i| ((i * 91) as i16).wrapping_sub((i * 7) as i16))
        .collect();
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_stereo() {
    let pcm: Vec<i16> = (0..2048)
        .map(|i| {
            if i % 2 == 0 {
                ((i / 2) as i16).wrapping_mul(13)
            } else {
                -(((i / 2) as i16).wrapping_mul(13))
            }
        })
        .collect();
    let out = packer_round_trip(&pcm, 2, 44100);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_small_block_size() {
    let pcm: Vec<i16> = (0..1000)
        .map(|i| (i as i16).wrapping_mul(11))
        .collect();
    let mut buf = Vec::new();
    encode_pcm_packed_with_block_size(&pcm, 1, 22050, 8, &mut buf).unwrap();
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "packer_small_block",
    )
    .unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out, pcm);
}

#[test]
fn packer_typically_compresses_below_v1() {
    // For a signal with structure (silence + occasional pulses), the
    // packer's f_zero/Huffman books should produce a noticeably
    // smaller bitstream than v1's flat 16-bits-per-sample encoding.
    let mut pcm = vec![0i16; 4096];
    // Sparse pulses — long runs of zeros surround a few non-zero
    // samples.
    for (i, x) in pcm.iter_mut().enumerate() {
        if i % 64 == 0 {
            *x = 1;
        }
    }
    let mut v1 = Vec::new();
    encode_pcm(&pcm, 1, 22050, &mut v1).unwrap();
    let mut packed = Vec::new();
    encode_pcm_packed(&pcm, 1, 22050, &mut packed).unwrap();
    assert!(
        packed.len() < v1.len(),
        "packer should compress sparse signal: v1={} packed={}",
        v1.len(),
        packed.len()
    );
}

#[test]
fn round_trip_wav_input() {
    // Build a small in-memory RIFF WAV via hound, run it through
    // encode_wav, then decode through AcmDecoder and compare.
    let pcm: Vec<i16> = (0..4096)
        .map(|i| ((i as f32 * 0.05).sin() * 16000.0) as i16)
        .collect();
    let mut wav = Cursor::new(Vec::<u8>::new());
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 22050,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::new(&mut wav, spec).unwrap();
        for &s in &pcm {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
    }
    let wav_bytes = wav.into_inner();

    let mut acm = Vec::new();
    encode_wav(Cursor::new(wav_bytes), &mut acm).unwrap();

    let mut dec = AcmDecoder::open(
        &DataSource::new(acm),
        OutputChannels::Original,
        "wav_round_trip",
    )
    .unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out, pcm);
}
