use infinitier_mve_decoder::{MveDecoder, VideoFormat};
use sha2::Digest as _;
use std::io::Write as _;
use std::path::PathBuf;

fn iplogo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/IPLOGO.MVE")
}

/// Write all decoded frames as raw binary to `dest`.
/// Format per frame: u32-LE frame_idx, u16-LE width, u16-LE height, then width*height*4 RGBA bytes.
fn dump_frames(mve_path: impl AsRef<std::path::Path>, dest: impl AsRef<std::path::Path>) {
    let mut dec = MveDecoder::open(mve_path).expect("open failed");
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(dest).expect("create dest failed"),
    );
    let mut idx: u32 = 0;
    while let Some(frame) = dec.next_frame().expect("decode error") {
        f.write_all(&idx.to_le_bytes()).unwrap();
        f.write_all(&frame.video.width.to_le_bytes()).unwrap();
        f.write_all(&frame.video.height.to_le_bytes()).unwrap();
        f.write_all(&frame.video.pixels).unwrap();
        idx += 1;
    }
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
    let frame = dec
        .next_frame()
        .expect("decode error")
        .expect("no frames in file");
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
        frame1.video.pixels, frame2.video.pixels,
        "same file decoded twice should produce identical first frames"
    );
}

// ---------------------------------------------------------------------------
// Audio WAV output matches ffmpeg reference
// ---------------------------------------------------------------------------

/// Extracts audio from IPLOGO.MVE to `<workspace>/target/mve_audio_iplogo.wav`
/// via `MveDecoder::extract_audio_to_wav`, then verifies that the SHA-256 of
/// the raw PCM bytes matches the hash produced by:
///
///   ffmpeg -i IPLOGO.MVE /tmp/iplogo_ffmpeg.wav
///   python3 -c "
///       import struct, hashlib, pathlib
///       data = pathlib.Path('/tmp/iplogo_ffmpeg.wav').read_bytes()
///       i = 12
///       while data[i:i+4] != b'data': i += 8 + struct.unpack_from('<I', data, i+4)[0]
///       pcm = data[i+8 : i+8 + struct.unpack_from('<I', data, i+4)[0]]
///       print(hashlib.sha256(pcm).hexdigest())
///   "
///
/// ffmpeg stops before the trailing post-video AUDIO_SILENCE chunks present
/// in the MVE file (mask = 0xffff, all-zero samples).  The test therefore
/// compares only the first FFMPEG_SAMPLE_COUNT samples, which are
/// byte-for-byte identical to ffmpeg's output.
#[test]
fn audio_wav_matches_ffmpeg_hash() {
    // Number of interleaved i16 samples in ffmpeg's reference output.
    // Verified with: ffprobe -show_streams /tmp/iplogo_ffmpeg.wav
    //   => 22050 Hz, stereo, 15.33 s  →  676800 samples
    const FFMPEG_SAMPLE_COUNT: usize = 676800;

    // SHA-256 of the raw PCM bytes (little-endian i16, no WAV header) from
    // `ffmpeg -i IPLOGO.MVE /tmp/iplogo_ffmpeg.wav`.
    const EXPECTED_SHA256: &str =
        "173bef927c8e63652282e4f8ebefdadd67fc66df2bf748390cb8a6add9f305e1";

    // ---- write WAV via MveDecoder::extract_audio_to_wav ----
    let wav_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&wav_dir).expect("could not create target dir");
    let wav_path = wav_dir.join("mve_audio_iplogo.wav");

    MveDecoder::open(iplogo_path())
        .expect("failed to open IPLOGO.MVE")
        .extract_audio_to_wav(&wav_path)
        .expect("extract_audio_to_wav failed");

    // ---- read the WAV back and collect PCM samples ----
    let mut reader = hound::WavReader::open(&wav_path).expect("failed to open written WAV");
    let all_samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .expect("failed to read WAV samples");

    assert!(
        all_samples.len() >= FFMPEG_SAMPLE_COUNT,
        "expected at least {FFMPEG_SAMPLE_COUNT} samples, got {}",
        all_samples.len()
    );

    // ---- hash the first FFMPEG_SAMPLE_COUNT samples and compare ----
    // We hash only the portion that ffmpeg outputs (trailing post-video
    // silence is excluded).  The hash covers the raw little-endian i16 bytes,
    // identical to the bytes in the WAV data chunk, so it is independent of
    // the WAV header written by hound.
    let mut hasher = sha2::Sha256::new();
    for &s in &all_samples[..FFMPEG_SAMPLE_COUNT] {
        hasher.update(s.to_le_bytes());
    }
    let hash_bytes = hasher.finalize();
    let hex: String = hash_bytes.iter().map(|b| format!("{b:02x}")).collect();

    assert_eq!(
        hex,
        EXPECTED_SHA256,
        "PCM hash mismatch.\n  \
         WAV written to: {}\n  \
         Expected (ffmpeg): {EXPECTED_SHA256}\n  \
         Got:               {hex}",
        wav_path.display()
    );
}

// ---------------------------------------------------------------------------
// Frame-by-frame comparison against the C reference decoder (GemRB)
// ---------------------------------------------------------------------------

/// Dumps all Rust-decoded frames to /tmp/rust_frames.bin in the same binary
/// format as the C `mve_dump` tool, so they can be compared byte-for-byte.
#[test]
fn dump_frames_for_c_comparison() {
    dump_frames(iplogo_path(), "/tmp/rust_frames.bin");
    // If the file was written without panicking, the basic decode succeeded.
    let meta = std::fs::metadata("/tmp/rust_frames.bin").expect("output file missing");
    assert!(meta.len() > 0, "output file is empty");
}

/// Compares the Rust-decoded frames byte-for-byte with the C-decoded frames.
///
/// Pre-condition: /tmp/c_frames.bin must exist (produced by the `mve_dump` C
/// tool built in `tools/mve_dump.cpp`).  The test is skipped with a clear
/// message if that file is absent.
#[test]
fn frames_match_c_reference() {
    let c_path = std::path::Path::new("/tmp/c_frames.bin");
    if !c_path.exists() {
        eprintln!("SKIP: /tmp/c_frames.bin not found — build tools/mve_dump.cpp and run:\n  \
                   tools/mve_dump src/mve_decoder/tests/resources/IPLOGO.MVE > /tmp/c_frames.bin");
        return;
    }

    // Decode all frames with the Rust decoder
    let mut dec = MveDecoder::open(iplogo_path()).expect("open failed");
    let mut rust_frames: Vec<(u32, u16, u16, Vec<u8>)> = Vec::new();
    let mut idx: u32 = 0;
    while let Some(frame) = dec.next_frame().expect("decode error") {
        rust_frames.push((idx, frame.video.width, frame.video.height, frame.video.pixels));
        idx += 1;
    }

    // Parse the C reference file
    let c_data = std::fs::read(c_path).expect("read c_frames.bin failed");
    let mut pos = 0usize;
    let mut c_frame_idx = 0u32;

    while pos < c_data.len() {
        assert!(
            pos + 8 <= c_data.len(),
            "C frame file truncated at byte {pos}"
        );
        let c_idx = u32::from_le_bytes(c_data[pos..pos + 4].try_into().unwrap());
        let c_w   = u16::from_le_bytes(c_data[pos + 4..pos + 6].try_into().unwrap());
        let c_h   = u16::from_le_bytes(c_data[pos + 6..pos + 8].try_into().unwrap());
        pos += 8;
        let pixel_bytes = c_w as usize * c_h as usize * 4;
        assert!(
            pos + pixel_bytes <= c_data.len(),
            "C frame {c_idx} pixel data truncated"
        );
        let c_pixels = &c_data[pos..pos + pixel_bytes];
        pos += pixel_bytes;

        assert!(
            (c_frame_idx as usize) < rust_frames.len(),
            "C has more frames ({c_idx}) than Rust ({})",
            rust_frames.len()
        );
        let (r_idx, r_w, r_h, ref r_pixels) = rust_frames[c_frame_idx as usize];

        assert_eq!(c_idx, r_idx, "frame index mismatch");
        assert_eq!(c_w, r_w, "frame {c_idx} width mismatch: C={c_w} Rust={r_w}");
        assert_eq!(c_h, r_h, "frame {c_idx} height mismatch: C={c_h} Rust={r_h}");
        assert_eq!(
            c_pixels, r_pixels.as_slice(),
            "frame {c_idx} pixel data differs ({}×{} = {} bytes)",
            c_w, c_h, pixel_bytes
        );

        c_frame_idx += 1;
    }

    assert_eq!(
        c_frame_idx as usize,
        rust_frames.len(),
        "Rust decoded {} frames but C decoded {}",
        rust_frames.len(),
        c_frame_idx
    );

    println!("All {c_frame_idx} frames match between C and Rust decoders.");
}
