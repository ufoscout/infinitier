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
    let dec = open("resources/MVE/8_bits/BILOGO.MVE", "BILOGO.MVE");
    assert_eq!(dec.format(), MovieFormat::Mve);
    let info = dec.info();
    println!(
        "BILOGO: {}x{}, frame_dur={}us, total={:?}",
        info.width, info.height, info.frame_duration_us, info.total_duration_us,
    );
    assert!(info.width > 0, "width must be non-zero");
    assert!(info.height > 0, "height must be non-zero");
    assert!(info.frame_duration_us > 0, "frame_dur must be non-zero");
    let total = info.total_duration_us;
    assert_eq!(
        total % info.frame_duration_us as u64,
        0,
        "total must be a whole multiple of frame_duration_us",
    );
    let frame_count = total / info.frame_duration_us as u64;
    assert!(frame_count > 0, "at least one frame must be present");

    // Pull a frame and verify its shape against the cached info.
    let mut dec = dec;
    let frame = dec
        .next_frame()
        .expect("next_frame")
        .expect("first frame must exist");
    assert_eq!(frame.video.width, info.width);
    assert_eq!(frame.video.height, info.height);
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
        "logo_lucas: {}x{}, frame_dur={}us, total={:?}",
        info.width, info.height, info.frame_duration_us, info.total_duration_us,
    );
    assert!(info.width > 0, "width must be non-zero");
    assert!(info.height > 0, "height must be non-zero");
    assert!(info.frame_duration_us > 0, "frame_dur must be non-zero");
    // BIK headers carry an explicit frame count → total duration is
    // computable up front.
    let total = info.total_duration_us;
    assert_eq!(
        total,
        266 * info.frame_duration_us as u64,
        "266 frames @ frame_dur"
    );

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
fn movie_source_opens_bundled_wbm() {
    let mut dec = open("resources/WBM/logo.wbm", "logo.wbm");
    assert_eq!(dec.format(), MovieFormat::Wbm);
    let info = dec.info();
    println!(
        "logo.wbm: {}x{}, frame_dur={}us, total={:?}us",
        info.width, info.height, info.frame_duration_us, info.total_duration_us,
    );
    assert!(info.width > 0, "width must be non-zero");
    assert!(info.height > 0, "height must be non-zero");
    assert!(
        info.frame_duration_us > 0,
        "WebM track DefaultDuration must be present",
    );

    // Pull a handful of frames to exercise both the VP8 path (motion
    // compensation kicks in after the first keyframe) and the Vorbis
    // path through the unified dispatcher.
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
                    "RGBA size must match w*h*4",
                );
                audio_chunks += frame.audio.len();
                audio_samples += frame.audio.iter().map(|c| c.samples.len()).sum::<usize>();
                decoded += 1;
            }
            None => break,
        }
    }
    assert!(decoded > 0, "should decode at least one frame");
    assert!(audio_chunks > 0, "WBM logo carries a Vorbis track");
    println!(
        "logo.wbm: decoded {decoded} frames, {audio_chunks} audio chunks, {audio_samples} samples",
    );
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
