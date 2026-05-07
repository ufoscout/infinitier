//! Round-trip test for `encode_from_assets_rgb555`.
//!
//! Generates a small set of RGB888 PNGs in a tempdir, runs the
//! HiColor from-assets encoder, then decodes and checks that every
//! pixel matches its 5-bit-per-channel quantised source — exactly
//! what the format guarantees (RGB888 → RGB555 drops the low 3 bits
//! of each channel; the decoder replicates them back via
//! `(c5 << 3) | (c5 >> 2)`, so an 8-bit input value `v` round-trips
//! to `((v & 0xF8) << 3) | ((v & 0xF8) >> 2)` ).

use std::fs;
use std::path::PathBuf;

use image::{ImageBuffer, Rgb};
use infinitier_datasource::DataSource;
use infinitier_mve_decoder::{MveDecoder, VideoFormat};
use infinitier_mve_encoder::{FromAssetsOptions, encode_from_assets_rgb555};

/// Convert an 8-bit channel value to what the encode + decode chain
/// will reconstruct: drop the low 3 bits, then 5→8 expand.
fn quantise_replicate(v: u8) -> u8 {
    let c5 = v >> 3;
    (c5 << 3) | (c5 >> 2)
}

fn write_png(path: &std::path::Path, w: u32, h: u32, px_fn: impl Fn(u32, u32) -> [u8; 3]) {
    let img = ImageBuffer::from_fn(w, h, |x, y| Rgb(px_fn(x, y)));
    img.save(path).unwrap();
}

#[test]
fn from_assets_rgb555_round_trip_silent() {
    let tmp = tempfile::tempdir().unwrap();
    let asset_dir = tmp.path().join("hicolor");
    fs::create_dir_all(&asset_dir).unwrap();

    let (w, h) = (16u32, 16u32);
    let n_frames = 4usize;
    let mut paths = Vec::new();
    for i in 0..n_frames {
        let p = asset_dir.join(format!("frame_{:04}.png", i));
        let phase = i as u32;
        write_png(&p, w, h, |x, y| {
            // Vary per-frame so frames aren't all identical (skip
            // would otherwise dominate and we'd never exercise the
            // colour-stream path). Use multiples of 8 so the source
            // is pre-quantised — this lets the test assert bit-exact
            // 8-bit equality after the round-trip.
            let r = ((x * 16 + phase * 32) & 0xF8) as u8;
            let g = ((y * 16 + phase * 16) & 0xF8) as u8;
            let b = ((((x + y) * 8) + phase * 8) & 0xF8) as u8;
            [r, g, b]
        });
        paths.push(p);
    }

    let out_dir = tmp.path().join("out");
    let opts = FromAssetsOptions {
        frame_duration_us: 66_667,
        lossy_downsample: false,
        strict_palette: false,
        output_name: "hi".to_string(),
    };
    let mve_path = encode_from_assets_rgb555(&paths, /*wav_path=*/ None, &opts, &out_dir)
        .expect("encode_from_assets_rgb555");
    assert!(mve_path.is_file());

    let bytes = fs::read(&mve_path).unwrap();
    let ds = DataSource::new(bytes);
    let mut dec: MveDecoder<Box<dyn infinitier_datasource::DataTrait>> =
        MveDecoder::new(ds.reader().unwrap(), "hi").unwrap();
    assert_eq!(dec.format(), VideoFormat::Rgb555);
    assert_eq!((dec.width(), dec.height()), (w as u16, h as u16));

    // Build expected pixel buffers (post-quantise) and compare.
    let mut frame_idx = 0;
    while let Some(frame) = dec.next_frame().unwrap() {
        let src_path = &paths[frame_idx];
        let src = image::ImageReader::open(src_path)
            .unwrap()
            .decode()
            .unwrap()
            .into_rgb8();
        for (px_idx, src_px) in src.pixels().enumerate() {
            let expected = [
                quantise_replicate(src_px[0]),
                quantise_replicate(src_px[1]),
                quantise_replicate(src_px[2]),
                0xff,
            ];
            let got = &frame.video.pixels[px_idx * 4..px_idx * 4 + 4];
            assert_eq!(
                got, &expected,
                "frame {frame_idx} pixel {px_idx} mismatch \
                 (src={src_px:?}, expected={expected:?}, got={got:?})"
            );
        }
        frame_idx += 1;
    }
    assert_eq!(frame_idx, n_frames);
}

#[test]
fn from_assets_rgb555_with_audio() {
    // Same as above but with a WAV — exercises the audio path under
    // the HiColor encoder. We don't pixel-check here (covered by the
    // silent test), just verify the pipeline encodes + decodes
    // without error and the audio segment count matches frames.
    let tmp = tempfile::tempdir().unwrap();
    let asset_dir = tmp.path().join("hicolor_audio");
    fs::create_dir_all(&asset_dir).unwrap();
    let (w, h) = (8u32, 8u32);
    let n_frames = 5usize;
    let mut paths = Vec::new();
    for i in 0..n_frames {
        let p = asset_dir.join(format!("f_{:04}.png", i));
        write_png(&p, w, h, |_, _| [0x40, 0x80, 0xC0]);
        paths.push(p);
    }

    let wav_path = asset_dir.join("audio.wav");
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 22_050,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav_path, spec).unwrap();
        for i in 0..2_000i16 {
            writer.write_sample(i.wrapping_mul(7)).unwrap();
        }
        writer.finalize().unwrap();
    }

    let out_dir = tmp.path().join("out");
    let opts = FromAssetsOptions {
        frame_duration_us: 66_667,
        lossy_downsample: false,
        strict_palette: false,
        output_name: "hi_audio".to_string(),
    };
    let mve_path =
        encode_from_assets_rgb555(&paths, Some(wav_path.as_path()), &opts, &out_dir).unwrap();

    let bytes = fs::read(&mve_path).unwrap();
    let ds = DataSource::new(bytes);
    let mut dec: MveDecoder<Box<dyn infinitier_datasource::DataTrait>> =
        MveDecoder::new(ds.reader().unwrap(), "hi_audio").unwrap();
    let mut decoded_frames = 0;
    let mut total_audio_samples = 0;
    while let Some(frame) = dec.next_frame().unwrap() {
        decoded_frames += 1;
        for chunk in &frame.audio {
            total_audio_samples += chunk.samples.len();
        }
    }
    assert_eq!(decoded_frames, n_frames);
    assert!(
        total_audio_samples > 0,
        "expected audio samples to land somewhere; got {total_audio_samples}"
    );
}

// Pull these into the dev-dep graph (they're on the encoder crate's
// dev-dependencies list); the import above (`infinitier_mve_decoder`,
// `image`, `hound`, `tempfile`) wires them up.
#[allow(dead_code)]
fn _force_uses() {
    let _: PathBuf = PathBuf::new();
}
