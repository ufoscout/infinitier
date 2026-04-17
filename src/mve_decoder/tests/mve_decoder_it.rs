use std::io::Cursor;
use std::path::PathBuf;

use infinitier_mve_decoder::{MveDecoder, VideoFormat};
use sha2::Digest as _;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = sha2::Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn samples_to_wav_bytes(samples: &[i16], channels: u8, sample_rate: u32) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::<u8>::new());
    let spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for &s in samples {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}

/// Decodes an MVE file and asserts every decoded value matches the companion JSON.
fn assert_matches_json(mve_rel: &str, json_rel: &str) {
    let mve_path  = manifest_dir().join(mve_rel);
    let json_path = manifest_dir().join(json_rel);

    // ---- Parse JSON ----
    let raw = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", json_path.display()));
    let j: serde_json::Value = serde_json::from_str(&raw).unwrap();

    let exp_width          = j["video"]["width"].as_u64().unwrap() as u16;
    let exp_height         = j["video"]["height"].as_u64().unwrap() as u16;
    let exp_palette_bits   = j["video"]["palette_bits"].as_u64().unwrap() as u8;
    let exp_frame_count    = j["video"]["frame_count"].as_u64().unwrap() as usize;
    let exp_frame_dur_us   = j["video"]["frame_duration_us"].as_u64().unwrap() as u32;
    let exp_frame_hashes: Vec<&str> = j["video"]["frame_hashes"]
        .as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap()).collect();

    let exp_channels        = j["audio"]["channels"].as_u64().unwrap() as u8;
    let exp_sample_rate     = j["audio"]["sample_rate"].as_u64().unwrap() as u32;
    let exp_bits_per_sample = j["audio"]["bits_per_sample"].as_u64().unwrap() as u16;
    let exp_total_samples   = j["audio"]["total_samples"].as_u64().unwrap() as usize;
    let exp_wav_sha256      = j["audio"]["wav_sha256"].as_str().unwrap();

    // ---- Open decoder and check static metadata ----
    let mut dec = MveDecoder::open(&mve_path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", mve_path.display()));

    assert_eq!(dec.width(),  exp_width,  "video.width");
    assert_eq!(dec.height(), exp_height, "video.height");

    let expected_format = match exp_palette_bits {
        8  => VideoFormat::Palette8,
        16 => VideoFormat::Rgb555,
        n  => panic!("unexpected palette_bits {n}"),
    };
    assert_eq!(dec.format(), expected_format, "video.palette_bits → format");

    // ---- Decode all frames ----
    let mut frame_hashes: Vec<String> = Vec::new();
    let mut all_samples: Vec<i16>     = Vec::new();
    let mut audio_channels: Option<u8>   = None;
    let mut audio_sample_rate: Option<u32> = None;
    // OC_CREATE_TIMER lives in the first video chunk, so duration_us is 0
    // until the first frame is decoded; after that it is constant.
    let mut frame_dur_us: u32 = 0;

    while let Some(frame) = dec.next_frame().expect("decode error") {
        frame_dur_us = frame.video.duration_us;
        frame_hashes.push(sha256_hex(&frame.video.pixels));
        for chunk in &frame.audio {
            if audio_channels.is_none() {
                audio_channels    = Some(chunk.channels);
                audio_sample_rate = Some(chunk.sample_rate);
            }
            all_samples.extend_from_slice(&chunk.samples);
        }
    }

    // ---- Verify video ----
    assert_eq!(frame_dur_us, exp_frame_dur_us, "video.frame_duration_us");
    assert_eq!(frame_hashes.len(), exp_frame_count, "video.frame_count");
    for (i, (got, exp)) in frame_hashes.iter().zip(exp_frame_hashes.iter()).enumerate() {
        assert_eq!(got.as_str(), *exp, "video.frame_hashes[{i}]");
    }

    // ---- Verify audio ----
    let channels    = audio_channels.expect("no audio decoded");
    let sample_rate = audio_sample_rate.unwrap();

    assert_eq!(channels,          exp_channels,        "audio.channels");
    assert_eq!(sample_rate,       exp_sample_rate,     "audio.sample_rate");
    assert_eq!(16u16,             exp_bits_per_sample, "audio.bits_per_sample");
    assert_eq!(all_samples.len(), exp_total_samples,   "audio.total_samples");

    let wav_bytes = samples_to_wav_bytes(&all_samples, channels, sample_rate);
    let wav_hash  = sha256_hex(&wav_bytes);
    assert_eq!(wav_hash.as_str(), exp_wav_sha256, "audio.wav_sha256");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_decoding_palette8_video_and_audio() {
    assert_matches_json(
        "tests/resources/8_bits/BILOGO.MVE",
        "tests/resources/8_bits/BILOGO.json",
    );
}

#[test]
fn test_decoding_palette16_video_and_audio() {
    assert_matches_json(
        "tests/resources/16_bits/BISLOGO.MVE",
        "tests/resources/16_bits/BISLOGO.json",
    );
}
