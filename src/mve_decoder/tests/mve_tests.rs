use infinitier_mve_decoder::{MveDecoder, VideoFormat};
use std::path::PathBuf;

fn iplogo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/resources/IPLOGO.MVE")
}

// ---------------------------------------------------------------------------
// Basic sanity: can we open and read the file?
// ---------------------------------------------------------------------------

#[test]
fn opens_without_error() {
    MveDecoder::open(iplogo_path()).expect("failed to open IPLOGO.MVE");
}

// ---------------------------------------------------------------------------
// Video metadata
// ---------------------------------------------------------------------------

#[test]
fn video_dimensions_are_nonzero() {
    let dec = MveDecoder::open(iplogo_path()).unwrap();
    assert!(dec.width() > 0, "width should be > 0");
    assert!(dec.height() > 0, "height should be > 0");
    // Width and height must be multiples of 8 (block size)
    assert_eq!(dec.width() % 8, 0);
    assert_eq!(dec.height() % 8, 0);
}

#[test]
fn frame_duration_is_positive() {
    // CREATE_TIMER lives in the first video chunk, so decode one frame first.
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    let frame = dec.next_frame().unwrap().unwrap();
    assert!(
        frame.video.duration_us > 0,
        "frame duration_us should be positive, got {}",
        frame.video.duration_us
    );
}

#[test]
fn video_format_is_detected() {
    let dec = MveDecoder::open(iplogo_path()).unwrap();
    // BG1/BG2 MVE files are 8-bit paletted
    assert_eq!(dec.format(), VideoFormat::Palette8);
}

// ---------------------------------------------------------------------------
// Frame decoding
// ---------------------------------------------------------------------------

#[test]
fn decodes_at_least_one_frame() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    let frame = dec.next_frame().expect("decode error").expect("no frames in file");
    assert_eq!(frame.video.width, dec.width());
    assert_eq!(frame.video.height, dec.height());
}

#[test]
fn first_frame_pixel_buffer_has_correct_size() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    let frame = dec.next_frame().unwrap().unwrap();
    let expected = frame.video.width as usize * frame.video.height as usize * 4;
    assert_eq!(
        frame.video.pixels.len(),
        expected,
        "pixel buffer should be width * height * 4 (RGBA)"
    );
}

#[test]
fn pixels_are_valid_rgba() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    let frame = dec.next_frame().unwrap().unwrap();
    // All alpha values in the RGBA buffer should be 0xFF
    for (i, chunk) in frame.video.pixels.chunks_exact(4).enumerate() {
        assert_eq!(chunk[3], 0xff, "pixel {i} alpha should be 0xFF");
    }
}

#[test]
fn decodes_multiple_frames() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    let mut count = 0u32;
    while let Some(_frame) = dec.next_frame().expect("decode error") {
        count += 1;
        if count > 10 {
            break;
        }
    }
    assert!(count >= 1, "should decode at least 1 frame");
}

#[test]
fn decodes_all_frames_without_error() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    let mut count = 0u32;
    while let Some(_frame) = dec.next_frame().expect("decode error") {
        count += 1;
    }
    assert!(count > 0, "should decode at least one frame");
    println!("Total frames in IPLOGO.MVE: {count}");
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

#[test]
fn audio_chunk_has_correct_channel_count() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    // Look through a few frames for audio data
    for _ in 0..20 {
        if let Some(frame) = dec.next_frame().expect("decode error") {
            for chunk in &frame.audio {
                assert!(
                    chunk.channels == 1 || chunk.channels == 2,
                    "channels should be 1 or 2, got {}",
                    chunk.channels
                );
                assert!(chunk.sample_rate > 0, "sample rate should be positive");
            }
        } else {
            break;
        }
    }
}

#[test]
fn audio_samples_are_within_i16_range() {
    let mut dec = MveDecoder::open(iplogo_path()).unwrap();
    // All samples must fit in i16 (they already are i16, just sanity-check they're not all zero)
    let mut found_nonzero = false;
    'outer: for _ in 0..30 {
        if let Some(frame) = dec.next_frame().expect("decode error") {
            for chunk in &frame.audio {
                for &s in &chunk.samples {
                    if s != 0 {
                        found_nonzero = true;
                        break 'outer;
                    }
                }
            }
        } else {
            break;
        }
    }
    assert!(found_nonzero, "expected some non-zero audio samples");
}

// ---------------------------------------------------------------------------
// Determinism: decoding the same file twice gives the same first frame
// ---------------------------------------------------------------------------

#[test]
fn decoding_is_deterministic() {
    let mut dec1 = MveDecoder::open(iplogo_path()).unwrap();
    let mut dec2 = MveDecoder::open(iplogo_path()).unwrap();

    let frame1 = dec1.next_frame().unwrap().unwrap();
    let frame2 = dec2.next_frame().unwrap().unwrap();

    assert_eq!(
        frame1.video.pixels,
        frame2.video.pixels,
        "same file decoded twice should produce identical first frames"
    );
}
