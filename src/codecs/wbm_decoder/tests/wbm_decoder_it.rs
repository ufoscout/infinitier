//! WBM corpus regression: decode the bundled `assets/WBM/logo.wbm`
//! and verify against the sibling `logo.json` fixture.
//!
//! ## How the fixture was produced
//!
//! `logo.json` was authored once, off-line, with no ffmpeg involvement
//! at test time:
//!
//! * `video.frame_hashes` come from `ffmpeg -f rawvideo -pix_fmt
//!   yuv420p` — external bit-exact ground truth. Our VP8 path matches
//!   libvpx byte-for-byte across the full stream including all P-frame
//!   inter-prediction, so a strict per-frame SHA-256 comparison is
//!   the strongest correctness signal we can record.
//!
//! * `audio.total_samples` is recorded as our pipeline (`lewton`)
//!   produces, *not* libvorbis: the two implementations end up at
//!   slightly different totals because `lewton::audio::read_audio_packet`
//!   is fed raw Vorbis packets out of a Matroska container — it never
//!   sees the Ogg `granule_pos` that libvorbis uses to trim the
//!   stream-end overlap window. The audio is otherwise audibly
//!   identical (±1 LSB IMDCT FP rounding scatter, ~50% of samples).
//!   No PCM hash is recorded for the same reason.

use std::fs::File;
use std::io::BufReader;

use infinitier_test_utils::{get_assets_path, parse_json_file};
use infinitier_wbm_decoder::{WbmOutputFormat, WbmPixels, WbmStreamingDecoder};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct WbmFixture {
    video: VideoFixture,
    audio: Option<AudioFixture>,
}

#[derive(Debug, Deserialize)]
struct VideoFixture {
    codec_tag: String,
    container: String,
    width: u32,
    height: u32,
    frame_count: u32,
    frame_duration_us: u64,
    frame_hashes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AudioFixture {
    codec: String,
    channels: u32,
    sample_rate: u32,
    bits_per_sample: u32,
    format: String,
    total_samples: u64,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Pack tight-stride YUV planes into the same byte layout `ffmpeg
/// -f rawvideo -pix_fmt yuv420p` produces, so the per-frame hashes
/// match the fixture.
fn pack_yuv420p(
    y: &[u8],
    u: &[u8],
    v: &[u8],
    y_stride: usize,
    uv_stride: usize,
    w: usize,
    h: usize,
) -> Vec<u8> {
    let cw = w.div_ceil(2);
    let ch = h.div_ceil(2);
    let mut out = Vec::with_capacity(w * h + 2 * cw * ch);
    for row in 0..h {
        out.extend_from_slice(&y[row * y_stride..row * y_stride + w]);
    }
    for row in 0..ch {
        out.extend_from_slice(&u[row * uv_stride..row * uv_stride + cw]);
    }
    for row in 0..ch {
        out.extend_from_slice(&v[row * uv_stride..row * uv_stride + cw]);
    }
    out
}

#[test]
fn logo_wbm_matches_fixture() {
    let asset = get_assets_path().join("WBM/logo.wbm");
    let json = get_assets_path().join("WBM/logo.json");
    assert!(asset.is_file(), "missing {}", asset.display());
    assert!(json.is_file(), "missing {}", json.display());

    let fixture: WbmFixture = parse_json_file(&json);

    // ---- Static metadata ----
    assert_eq!(fixture.video.codec_tag, "vp8");
    assert_eq!(fixture.video.container, "matroska/webm");

    let f = BufReader::new(File::open(&asset).expect("open wbm"));
    let mut decoder =
        WbmStreamingDecoder::new(f, "logo.wbm").expect("WbmStreamingDecoder construction");
    decoder = decoder.with_output_format(WbmOutputFormat::Yuv);

    assert_eq!(decoder.width(), fixture.video.width, "video.width");
    assert_eq!(decoder.height(), fixture.video.height, "video.height");
    assert_eq!(
        decoder.frame_duration_us() as u64,
        fixture.video.frame_duration_us,
        "video.frame_duration_us",
    );
    assert_eq!(decoder.has_audio(), fixture.audio.is_some());

    // ---- Decode every frame ----
    let w = decoder.width() as usize;
    let h = decoder.height() as usize;
    let mut got_frames: Vec<String> = Vec::with_capacity(fixture.video.frame_hashes.len());
    let mut audio_total_samples: u64 = 0;
    let mut audio_channels: Option<u8> = None;
    let mut audio_rate: Option<u32> = None;

    while let Some(frame) = decoder.next_frame().expect("decode error") {
        // Capture audio metadata as we see it — lewton initialises
        // these from the Vorbis ident header, so values are stable
        // across all chunks.
        for chunk in &frame.audio {
            audio_channels.get_or_insert(chunk.channels);
            audio_rate.get_or_insert(chunk.sample_rate);
            audio_total_samples += chunk.samples.len() as u64;
        }

        let yuv = match &frame.video.pixels {
            WbmPixels::Yuv(p) => p,
            WbmPixels::Rgba(_) => panic!("requested YUV but got RGBA"),
        };
        let packed = pack_yuv420p(
            &yuv.y,
            &yuv.u,
            &yuv.v,
            yuv.y_stride as usize,
            yuv.uv_stride as usize,
            w,
            h,
        );
        got_frames.push(sha256_hex(&packed));
    }

    // ---- Verify video ----
    assert_eq!(
        got_frames.len(),
        fixture.video.frame_count as usize,
        "frame_count: decoded {} vs fixture {}",
        got_frames.len(),
        fixture.video.frame_count,
    );
    for (i, (got, want)) in got_frames
        .iter()
        .zip(fixture.video.frame_hashes.iter())
        .enumerate()
    {
        assert_eq!(got, want, "video.frame_hashes[{i}]");
    }

    // ---- Verify audio metadata ----
    if let Some(exp_audio) = fixture.audio.as_ref() {
        assert_eq!(exp_audio.codec, "vorbis");
        assert_eq!(exp_audio.bits_per_sample, 16);
        assert_eq!(
            audio_channels.expect("no audio decoded") as u32,
            exp_audio.channels,
            "audio.channels",
        );
        assert_eq!(
            audio_rate.unwrap(),
            exp_audio.sample_rate,
            "audio.sample_rate",
        );
        let actual_format = format!(
            "PCM 16-bit {} at {} Hz",
            if exp_audio.channels == 2 { "stereo" } else { "mono" },
            exp_audio.sample_rate,
        );
        assert_eq!(actual_format, exp_audio.format, "audio.format");

        // The Vorbis IMDCT overlap-window means the very last decoded
        // packet may emit a few samples fewer or more depending on
        // whether the implementation produces the trailing window. We
        // accept ±1 packet's worth of slack (Vorbis typically emits
        // ~1024 samples per channel per packet at this bitrate).
        // ffmpeg + lewton both happen to land on the same per-channel
        // count for this fixture, so the assertion is exact today.
        assert_eq!(
            audio_total_samples,
            exp_audio.total_samples,
            "audio.total_samples (channels={})",
            exp_audio.channels,
        );
    } else {
        assert!(audio_channels.is_none(), "fixture says no audio but decoder produced some");
    }
}

#[test]
fn logo_wbm_rgba_output_shape() {
    // Sanity check the RGBA path: first frame must be `width*height*4`
    // bytes with all alpha channels = 255.
    let asset = get_assets_path().join("WBM/logo.wbm");
    let f = BufReader::new(File::open(&asset).expect("open wbm"));
    let mut decoder = WbmStreamingDecoder::new(f, "logo.wbm")
        .expect("decoder")
        .with_output_format(WbmOutputFormat::Rgba);

    let frame = decoder
        .next_frame()
        .expect("decode")
        .expect("at least one frame");
    let pixels = match &frame.video.pixels {
        WbmPixels::Rgba(p) => p,
        WbmPixels::Yuv(_) => panic!("expected RGBA"),
    };
    assert_eq!(
        pixels.len(),
        decoder.width() as usize * decoder.height() as usize * 4
    );
    assert!(
        pixels.chunks_exact(4).all(|px| px[3] == 255),
        "all alpha bytes must be 255",
    );
}
