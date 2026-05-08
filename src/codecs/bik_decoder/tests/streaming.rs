//! Integration test for [`BikStreamingDecoder`].
//!
//! For every entry in the corpus:
//! * Open with the default ([`BikOutputFormat::Yuv`]) and pull a few
//!   frames; verify dimensions match the fixture and audio chunks
//!   appear when the file has audio.
//! * Open again with [`BikOutputFormat::Rgba`] and check the RGBA
//!   buffer size matches `width * height * 4`.

mod common;

use std::fs::File;

use infinitier_bik_decoder::{BikOutputFormat, BikPixels, BikStreamingDecoder};

use crate::common::corpus;

#[test]
fn streaming_decoder_yuv_default_pulls_frames() {
    for entry in corpus() {
        let label = entry.label();
        let f = File::open(&entry.bik_path).expect("open");
        let mut dec = BikStreamingDecoder::new(f, label).expect("init");
        assert_eq!(dec.output_format(), BikOutputFormat::Yuv);
        assert_eq!(dec.width(), entry.fixture.video.width);
        assert_eq!(dec.height(), entry.fixture.video.height);
        assert_eq!(dec.frame_count(), entry.fixture.video.frame_count);

        let total_audio_samples_expected = entry
            .fixture
            .audio
            .as_ref()
            .map(|a| a.total_samples)
            .unwrap_or(0);

        let mut frames = 0usize;
        let mut audio_samples = 0u64;
        while let Some(frame) = dec.next_frame().expect("next_frame") {
            assert_eq!(frame.video.width, entry.fixture.video.width);
            assert_eq!(frame.video.height, entry.fixture.video.height);
            // The default config delivers YUV planes.
            match &frame.video.pixels {
                BikPixels::Yuv(planes) => {
                    assert_eq!(planes.y.width, frame.video.width);
                    assert_eq!(planes.y.height, frame.video.height);
                    // Chroma is half-resolution (4:2:0).
                    assert_eq!(planes.u.width, frame.video.width.div_ceil(2));
                    assert_eq!(planes.u.height, frame.video.height.div_ceil(2));
                }
                BikPixels::Rgba(_) => panic!("{label}: expected YUV pixels by default"),
            }
            for chunk in &frame.audio {
                audio_samples += chunk.samples.len() as u64;
            }
            frames += 1;
        }
        assert_eq!(frames as u32, entry.fixture.video.frame_count);
        assert_eq!(audio_samples, total_audio_samples_expected);
        eprintln!("✓  YUV  {:<24}  {} frames, {} audio samples", label, frames, audio_samples);
    }
}

#[test]
fn streaming_decoder_rgba_emits_rgba8() {
    for entry in corpus() {
        let label = entry.label();
        let f = File::open(&entry.bik_path).expect("open");
        let mut dec = BikStreamingDecoder::new(f, label)
            .expect("init")
            .with_output_format(BikOutputFormat::Rgba);
        assert_eq!(dec.output_format(), BikOutputFormat::Rgba);

        // Just probe the first 4 frames to keep the test cheap.
        let expected_bytes = entry.fixture.video.width as usize
            * entry.fixture.video.height as usize
            * 4;
        for i in 0..4u32 {
            match dec.next_frame().expect("next_frame") {
                Some(frame) => match &frame.video.pixels {
                    BikPixels::Rgba(pixels) => {
                        assert_eq!(
                            pixels.len(),
                            expected_bytes,
                            "{label} frame {i}: RGBA byte count must be w*h*4"
                        );
                    }
                    BikPixels::Yuv(_) => panic!("{label}: expected RGBA pixels"),
                },
                None => break,
            }
        }
        eprintln!("✓  RGBA {:<24}  ({} bytes/frame)", label, expected_bytes);
    }
}
