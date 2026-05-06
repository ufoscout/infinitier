//! Round-trip integration test for `encode_from_assets`.
//!
//! For every `assets/mve_encoder/<name>/` directory:
//!   1. Encode `frame_*.png` + `audio.wav` to `target/mve_encoder/<name>.mve`
//!   2. Decode the produced .mve via `infinitier_mve_decoder`
//!   3. Verify each decoded frame matches the source PNG (after the
//!      6-bit-per-channel palette quantisation that MVE storage forces)
//!   4. Verify the concatenated decoded audio matches the source WAV
//!      sample-for-sample.
//!
//! The 320x240_15fps_3s_noise asset is encoded with `lossy_downsample`;
//! that fixture relaxes the pixel check to "≤4-pixel-distance from the
//! 2×2 top-left, since the encoder discards the other 3". Audio is
//! always exact since 16-bit PCM round-trips losslessly.

use std::fs;
use std::path::{Path, PathBuf};

use hound::WavReader;
use image::ImageReader;
use infinitier_datasource::DataSource;
use infinitier_mve_decoder::MveDecoder;
use infinitier_mve_encoder::{encode_from_assets, FromAssetsOptions};

const ASSETS_ROOT: &str = "../../../assets/mve_encoder";
const OUTPUT_ROOT: &str = "../../../target/mve_encoder";

#[test]
fn round_trip_every_asset_folder() {
    let assets_root = manifest_relative(ASSETS_ROOT);
    let output_root = manifest_relative(OUTPUT_ROOT);
    fs::create_dir_all(&output_root).expect("create target/mve_encoder");

    let mut entries: Vec<PathBuf> = fs::read_dir(&assets_root)
        .unwrap_or_else(|e| panic!("reading {}: {e}", assets_root.display()))
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    assert!(
        !entries.is_empty(),
        "no asset folders under {}",
        assets_root.display()
    );

    for asset_dir in &entries {
        let name = asset_dir
            .file_name()
            .expect("asset folder name")
            .to_string_lossy()
            .into_owned();
        let png_paths = sorted_pngs(asset_dir);
        let wav_path = asset_dir.join("audio.wav");
        let frame_dur_us = parse_frame_duration_us_from_name(&name);
        let lossy = name.contains("noise");

        let opts = FromAssetsOptions {
            frame_duration_us: frame_dur_us,
            lossy_downsample: lossy,
            output_name: name.clone(),
        };
        let mve_path = encode_from_assets(&png_paths, &wav_path, &opts, &output_root)
            .unwrap_or_else(|e| panic!("encode_from_assets({name}): {e}"));
        assert!(mve_path.is_file(), "encoder did not write {mve_path:?}");

        verify_video(&png_paths, &mve_path, lossy);
        verify_audio(&wav_path, &mve_path);
        eprintln!("OK {name} ({} bytes)", fs::metadata(&mve_path).unwrap().len());
    }
}

fn manifest_relative(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn sorted_pngs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("png"))
        .collect();
    v.sort();
    assert!(!v.is_empty(), "no PNG frames in {}", dir.display());
    v
}

/// Asset folders are named `<W>x<H>_<N>fps_<S>s_<flavour>`. We use
/// the fps token to recover the encoder's `frame_duration_us`.
fn parse_frame_duration_us_from_name(name: &str) -> u32 {
    let fps: u32 = name
        .split('_')
        .find_map(|tok| tok.strip_suffix("fps").and_then(|n| n.parse().ok()))
        .unwrap_or_else(|| panic!("name {name} missing `<N>fps` token"));
    (1_000_000.0 / fps as f64).round() as u32
}

// ─── verification helpers ───────────────────────────────────────────────────

fn open_decoder(path: &Path) -> MveDecoder<Box<dyn infinitier_datasource::DataTrait>> {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let ds = DataSource::new(bytes);
    MveDecoder::new(ds.reader().unwrap(), path.to_string_lossy().into_owned())
        .unwrap_or_else(|e| panic!("open mve {path:?}: {e}"))
}

fn verify_video(png_paths: &[PathBuf], mve_path: &Path, lossy: bool) {
    let mut dec = open_decoder(mve_path);
    let (w, h) = (dec.width() as u32, dec.height() as u32);
    for (i, png) in png_paths.iter().enumerate() {
        let frame = dec
            .next_frame()
            .unwrap()
            .unwrap_or_else(|| panic!("decoder ran out at frame {i}"));
        let expected_rgb = ImageReader::open(png)
            .and_then(|r| Ok(r.with_guessed_format().unwrap()))
            .unwrap()
            .decode()
            .unwrap()
            .into_rgb8();
        assert_eq!(
            (frame.video.width as u32, frame.video.height as u32),
            (w, h)
        );

        if lossy {
            verify_frame_lossy(&expected_rgb, &frame.video.pixels, w, h, i);
        } else {
            verify_frame_lossless(&expected_rgb, &frame.video.pixels, w, h, i);
        }
    }
    assert!(
        dec.next_frame().unwrap().is_none(),
        "decoder produced more frames than the asset has PNGs"
    );
}

fn verify_frame_lossless(
    expected_rgb: &image::RgbImage,
    got_rgba: &[u8],
    w: u32,
    h: u32,
    frame_idx: usize,
) {
    for y in 0..h as usize {
        for x in 0..w as usize {
            let exp = expected_rgb.get_pixel(x as u32, y as u32);
            // MVE stores RGB as 6-bit per channel (>>2 on encode).
            let exp_q = [exp[0] & 0xfc, exp[1] & 0xfc, exp[2] & 0xfc, 255];
            let off = (y * w as usize + x) * 4;
            let got = &got_rgba[off..off + 4];
            assert_eq!(
                got, &exp_q,
                "frame {frame_idx} pixel ({x},{y}): got {got:?}, expected {exp_q:?}"
            );
        }
    }
}

fn verify_frame_lossy(
    expected_rgb: &image::RgbImage,
    got_rgba: &[u8],
    w: u32,
    h: u32,
    frame_idx: usize,
) {
    // With `lossy_downsample`, blocks that fell through to mode 0xc
    // emit one colour per 2×2 sub-block — the top-left pixel of each
    // 2×2 wins. So for every (x, y) we compare against the
    // expected pixel at (x & !1, y & !1).
    for y in 0..h as usize {
        for x in 0..w as usize {
            let sx = x & !1;
            let sy = y & !1;
            let exp = expected_rgb.get_pixel(sx as u32, sy as u32);
            let exp_q = [exp[0] & 0xfc, exp[1] & 0xfc, exp[2] & 0xfc, 255];
            let off = (y * w as usize + x) * 4;
            let got = &got_rgba[off..off + 4];
            // Allow exact match against either the 2×2-top-left pixel
            // (when 0xc fired) OR the actual pixel (when a cheaper
            // mode happened to fit the block losslessly).
            let exact = expected_rgb.get_pixel(x as u32, y as u32);
            let exact_q = [exact[0] & 0xfc, exact[1] & 0xfc, exact[2] & 0xfc, 255];
            assert!(
                got == exp_q || got == exact_q,
                "frame {frame_idx} pixel ({x},{y}): got {got:?}, expected {exp_q:?} or {exact_q:?}"
            );
        }
    }
}

fn verify_audio(wav_path: &Path, mve_path: &Path) {
    // Concatenate every audio chunk the decoder yields.
    let mut dec = open_decoder(mve_path);
    let mut decoded: Vec<i16> = Vec::new();
    let mut decoded_sample_rate: Option<u32> = None;
    let mut decoded_channels: Option<u8> = None;
    while let Some(frame) = dec.next_frame().unwrap() {
        for chunk in frame.audio {
            decoded_sample_rate.get_or_insert(chunk.sample_rate);
            decoded_channels.get_or_insert(chunk.channels);
            assert_eq!(
                decoded_sample_rate, Some(chunk.sample_rate),
                "audio chunk sample-rate changed mid-stream"
            );
            assert_eq!(
                decoded_channels, Some(chunk.channels),
                "audio chunk channels changed mid-stream"
            );
            decoded.extend_from_slice(&chunk.samples);
        }
    }

    let mut reader = WavReader::open(wav_path).expect("open wav");
    let spec = reader.spec();
    let expected: Vec<i16> = reader.samples::<i16>().collect::<Result<_, _>>().unwrap();

    assert_eq!(
        decoded_sample_rate,
        Some(spec.sample_rate),
        "sample-rate mismatch for {}",
        wav_path.display()
    );
    assert_eq!(
        decoded_channels.map(|c| c as u16),
        Some(spec.channels),
        "channel-count mismatch for {}",
        wav_path.display()
    );
    assert_eq!(
        decoded.len(),
        expected.len(),
        "audio sample-count mismatch for {} (got {}, expected {})",
        wav_path.display(),
        decoded.len(),
        expected.len()
    );
    assert_eq!(
        decoded, expected,
        "audio samples differ from source WAV for {}",
        wav_path.display()
    );
}
