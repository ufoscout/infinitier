//! Smoke tests for [`MovieSource`] / [`MovieDecoder`] against the
//! bundled MVE and BIK corpus assets. We open one of each, pull a few
//! frames, and assert the format dispatch and metadata are sane.

use infinitier_core::movie::{MovieDecoder, MovieFormat, MovieSource};
use infinitier_datasource::DataSource;
use infinitier_test_utils::get_assets_path;

fn open(path_in_assets: &str, label: &str) -> MovieDecoder {
    let path = get_assets_path().join(path_in_assets);
    let ds = DataSource::new(path.clone());
    let src = MovieSource::new(ds, label);
    src.open()
        .unwrap_or_else(|e| panic!("open {} failed: {e} (path={})", label, path.display()))
}

#[test]
fn movie_source_opens_bundled_mve() {
    let mut dec = open("resources/MVE/8_bits/BILOGO.MVE", "BILOGO.MVE");
    assert_eq!(dec.format(), MovieFormat::Mve);
    // For MVE the timer chunk lives in the first frame, so info() only
    // becomes fully populated after the first `next_frame`.
    let frame = dec
        .next_frame()
        .expect("next_frame")
        .expect("first frame must exist");
    let info = dec.info();
    println!(
        "BILOGO: {}x{}, frame_dur={}us",
        info.width, info.height, info.frame_duration_us
    );
    assert_eq!(frame.video.width, info.width, "frame width vs info");
    assert_eq!(frame.video.height, info.height, "frame height vs info");
    assert!(info.width > 0, "width must be non-zero");
    assert!(info.height > 0, "height must be non-zero");
    assert!(info.frame_duration_us > 0, "frame_dur must be non-zero");
    assert_eq!(
        frame.video.pixels.len(),
        info.width as usize * info.height as usize * 4,
        "RGBA size must match w*h*4",
    );
}

#[test]
fn movie_source_opens_bundled_bik() {
    let mut dec = open("resources/BIK/logo_lucas.bik", "logo_lucas.bik");
    assert_eq!(dec.format(), MovieFormat::Bik);
    // BIK has the timer up front in the header — no need to pull a
    // frame before reading info().
    let info = dec.info();
    println!(
        "logo_lucas: {}x{}, frame_dur={}us",
        info.width, info.height, info.frame_duration_us
    );
    assert!(info.width > 0, "width must be non-zero");
    assert!(info.height > 0, "height must be non-zero");
    assert!(info.frame_duration_us > 0, "frame_dur must be non-zero");

    // Pull a handful of frames so the audio/video paths both run.
    let mut decoded = 0usize;
    let mut audio_chunks = 0usize;
    let mut audio_samples = 0usize;
    for _ in 0..16 {
        match dec.next_frame().expect("next_frame") {
            Some(frame) => {
                assert_eq!(frame.video.width, info.width);
                assert_eq!(frame.video.height, info.height);
                assert_eq!(
                    frame.video.pixels.len(),
                    info.width as usize * info.height as usize * 4,
                );
                audio_chunks += frame.audio.len();
                audio_samples += frame.audio.iter().map(|c| c.samples.len()).sum::<usize>();
                decoded += 1;
            }
            None => break,
        }
    }
    assert!(decoded > 0, "should decode at least one frame");
    println!(
        "logo_lucas: decoded {decoded} frames, {audio_chunks} audio chunks, {audio_samples} samples"
    );
    // logo_lucas.bik ships with a stereo DCT audio track, so we expect
    // at least one chunk's worth of samples within the first 16 frames.
    assert!(audio_chunks > 0, "expected at least one audio chunk");
}

#[test]
fn movie_source_opens_bundled_bik_no_audio() {
    let mut dec = open("resources/BIK/logo_legal.bik", "logo_legal.bik");
    assert_eq!(dec.format(), MovieFormat::Bik);
    let info = dec.info();
    let frame = dec
        .next_frame()
        .expect("next_frame")
        .expect("frame must exist");
    assert_eq!(frame.video.width, info.width);
    assert_eq!(frame.video.height, info.height);
    assert!(frame.audio.is_empty(), "logo_legal has no audio track");
}
