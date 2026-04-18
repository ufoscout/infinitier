use std::io::Cursor;

use infinitier_datasource::DataSource;
use infinitier_mve_decoder::{MveDecoder, VideoFormat};
use infinitier_test_utils::get_assets_path;
use serde::Deserialize;
use sha2::Digest as _;

#[test]
fn test_decoding_palette8_video_and_audio() {
    assert_matches_json("8_bits/BILOGO.MVE", "8_bits/BILOGO.json");
}

#[test]
fn test_decoding_palette16_video_and_audio() {
    assert_matches_json("16_bits/BISLOGO.MVE", "16_bits/BISLOGO.json");
}

#[derive(Deserialize)]
struct MveReport {
    audio: AudioInfo,
    video: VideoInfo,
}

#[derive(Deserialize)]
struct AudioInfo {
    channels: u8,
    sample_rate: u32,
    bits_per_sample: u16,
    format: String,
    total_samples: usize,
    wav_sha256: String,
}

#[derive(Deserialize)]
struct VideoInfo {
    width: u16,
    height: u16,
    palette_bits: u8,
    frame_count: usize,
    frame_duration_us: u32,
    frame_hashes: Vec<String>,
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
    
    let mve_path = get_assets_path().join("resources/MVE").join(mve_rel);
    let json_path = get_assets_path().join("resources/MVE").join(json_rel);

    // ---- Parse JSON into typed struct ----
    let raw = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", json_path.display()));
    let report: MveReport = serde_json::from_str(&raw).unwrap();

    let exp_width = report.video.width;
    let exp_height = report.video.height;
    let exp_palette_bits = report.video.palette_bits;
    let exp_frame_count = report.video.frame_count;
    let exp_frame_dur_us = report.video.frame_duration_us;
    let exp_frame_hashes = &report.video.frame_hashes;

    let exp_channels = report.audio.channels;
    let exp_sample_rate = report.audio.sample_rate;
    let exp_bits_per_sample = report.audio.bits_per_sample;
    let exp_audio_format = &report.audio.format;
    let exp_total_samples = report.audio.total_samples;
    let exp_wav_sha256 = &report.audio.wav_sha256;

    // ---- Open decoder and check static metadata ----
    let data = DataSource::new(mve_path.clone());
    let reader = data.reader().unwrap();
    let mut dec = MveDecoder::new(reader)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", mve_path.display()));

    assert_eq!(dec.width(), exp_width, "video.width");
    assert_eq!(dec.height(), exp_height, "video.height");

    let expected_format = match exp_palette_bits {
        8 => VideoFormat::Palette8,
        16 => VideoFormat::Rgb555,
        n => panic!("unexpected palette_bits {n}"),
    };
    assert_eq!(dec.format(), expected_format, "video.palette_bits → format");

    // ---- Decode all frames ----
    let mut frame_hashes: Vec<String> = Vec::new();
    let mut all_samples: Vec<i16> = Vec::new();
    let mut audio_channels: Option<u8> = None;
    let mut audio_sample_rate: Option<u32> = None;
    // OC_CREATE_TIMER lives in the first video chunk, so duration_us is 0
    // until the first frame is decoded; after that it is constant.
    let mut frame_dur_us: u32 = 0;

    while let Some(frame) = dec.next_frame().expect("decode error") {
        frame_dur_us = frame.video.duration_us;
        frame_hashes.push(sha256_hex(&frame.video.pixels));
        for chunk in &frame.audio {
            if audio_channels.is_none() {
                audio_channels = Some(chunk.channels);
                audio_sample_rate = Some(chunk.sample_rate);
            }
            all_samples.extend_from_slice(&chunk.samples);
        }
    }

    // ---- Verify video ----
    assert_eq!(frame_dur_us, exp_frame_dur_us, "video.frame_duration_us");
    assert_eq!(frame_hashes.len(), exp_frame_count, "video.frame_count");
    for (i, (got, exp)) in frame_hashes.iter().zip(exp_frame_hashes.iter()).enumerate() {
        assert_eq!(got, exp, "video.frame_hashes[{i}]");
    }

    // ---- Verify audio ----
    let channels = audio_channels.expect("no audio decoded");
    let sample_rate = audio_sample_rate.unwrap();

    let actual_format = format!(
        "PCM 16-bit {} at {} Hz",
        if channels == 2 { "stereo" } else { "mono" },
        sample_rate,
    );
    assert_eq!(&actual_format, exp_audio_format, "audio.format");
    assert_eq!(channels, exp_channels, "audio.channels");
    assert_eq!(sample_rate, exp_sample_rate, "audio.sample_rate");
    assert_eq!(16u16, exp_bits_per_sample, "audio.bits_per_sample");
    assert_eq!(all_samples.len(), exp_total_samples, "audio.total_samples");

    let wav_bytes = samples_to_wav_bytes(&all_samples, channels, sample_rate);
    let wav_hash = sha256_hex(&wav_bytes);
    assert_eq!(&wav_hash, exp_wav_sha256, "audio.wav_sha256");
}
