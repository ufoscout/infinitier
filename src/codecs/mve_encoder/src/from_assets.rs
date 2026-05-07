//! High-level helper for encoding an MVE from a directory of frame
//! PNGs and a WAV audio track.
//!
//! Inputs:
//!   * `png_paths` — ordered list of PNG files, one per video frame.
//!     All frames must share the same width/height. Pixels are read
//!     as RGB and de-duplicated into a ≤256-entry palette; if the
//!     combined frames contain more than 256 unique colours the
//!     function returns [`FromAssetsError::TooManyColours`]. The
//!     companion asset generator runs ffmpeg's `palettegen` +
//!     `paletteuse` to ensure this constraint holds.
//!   * `wav_path` — 16-bit PCM WAV (mono or stereo). Sample rate must
//!     fit in a u16. The samples are split contiguously across the
//!     video frames in proportion to `frame_duration_us`.
//!   * `options` — frame timing + lossy fallback policy + output name.
//!   * `output_folder` — directory the resulting `.mve` is written
//!     into. Created if missing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use image::ImageReader;
use thiserror::Error;

use crate::{
    encode_av, AudioOptions, MveEncodeError, VideoOptions,
};

#[derive(Debug, Error)]
pub enum FromAssetsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image decode error for {path}: {source}")]
    Image {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
    #[error("wav decode error for {path}: {source}")]
    Wav {
        path: PathBuf,
        #[source]
        source: hound::Error,
    },
    #[error("png_paths is empty — at least one frame is required")]
    NoFrames,
    #[error("frame {index} ({path}) has size {got_w}x{got_h}; expected {exp_w}x{exp_h}")]
    FrameSizeMismatch {
        index: usize,
        path: PathBuf,
        got_w: u32,
        got_h: u32,
        exp_w: u32,
        exp_h: u32,
    },
    #[error(
        "wav has {bits}-bit samples; encoder requires 16-bit PCM"
    )]
    UnsupportedWavBitDepth { bits: u16 },
    #[error(
        "the frames contain more than 256 unique RGB colours \
         (the MVE palette caps at 256); regenerate the assets via \
         ffmpeg `palettegen=max_colors=256:reserve_transparent=0` \
         + `paletteuse=dither=none`"
    )]
    TooManyColours,
    #[error(transparent)]
    Encode(#[from] MveEncodeError),
}

/// Knobs for [`encode_from_assets`] independent of the asset files.
pub struct FromAssetsOptions {
    /// Microseconds between consecutive frames. e.g. 66667 ≈ 15 fps.
    pub frame_duration_us: u32,
    /// When `true`, blocks that would otherwise be emitted as raw
    /// 8×8 (`0xb`, 64 bytes) are instead lossily downsampled to
    /// `0xc` (16 bytes). Required for high-detail content whose
    /// raw form would exceed MVE's 65 535-byte segment cap.
    pub lossy_downsample: bool,
    /// Basename (no extension) for the produced `.mve` file in
    /// `output_folder`. e.g. `"smptebars"` → `<out>/smptebars.mve`.
    pub output_name: String,
}

/// Encode the given PNG sequence + WAV audio into an MVE file written
/// inside `output_folder`. Returns the path of the produced file.
pub fn encode_from_assets(
    png_paths: &[PathBuf],
    wav_path: &Path,
    options: &FromAssetsOptions,
    output_folder: &Path,
) -> Result<PathBuf, FromAssetsError> {
    if png_paths.is_empty() {
        return Err(FromAssetsError::NoFrames);
    }

    // 1. Read all PNGs as RGB.
    let mut rgb_frames: Vec<image::RgbImage> = Vec::with_capacity(png_paths.len());
    for path in png_paths {
        let img = ImageReader::open(path)
            .and_then(|r| Ok(r.with_guessed_format()?))
            .map_err(FromAssetsError::Io)?
            .decode()
            .map_err(|source| FromAssetsError::Image {
                path: path.clone(),
                source,
            })?
            .into_rgb8();
        rgb_frames.push(img);
    }

    let (width_u32, height_u32) = (rgb_frames[0].width(), rgb_frames[0].height());
    for (i, frame) in rgb_frames.iter().enumerate() {
        if frame.width() != width_u32 || frame.height() != height_u32 {
            return Err(FromAssetsError::FrameSizeMismatch {
                index: i,
                path: png_paths[i].clone(),
                got_w: frame.width(),
                got_h: frame.height(),
                exp_w: width_u32,
                exp_h: height_u32,
            });
        }
    }
    let width = width_u32 as u16;
    let height = height_u32 as u16;

    // 2. Build a shared palette and per-frame index buffers.
    let (palette, indexed_frames) = build_shared_palette(&rgb_frames)?;

    // 3. Read WAV samples.
    let mut reader = hound::WavReader::open(wav_path).map_err(|source| FromAssetsError::Wav {
        path: wav_path.to_path_buf(),
        source,
    })?;
    let spec = reader.spec();
    if spec.bits_per_sample != 16 {
        return Err(FromAssetsError::UnsupportedWavBitDepth {
            bits: spec.bits_per_sample,
        });
    }
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| FromAssetsError::Wav {
            path: wav_path.to_path_buf(),
            source,
        })?;

    let audio_opts = AudioOptions {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
    };
    let samples_per_frame = split_audio(&samples, indexed_frames.len(), spec.channels as usize);

    // 4. Encode.
    fs::create_dir_all(output_folder)?;
    let output_path = output_folder.join(format!("{}.mve", options.output_name));
    let mut out = fs::File::create(&output_path)?;

    let video_opts = VideoOptions {
        width,
        height,
        frame_duration_us: options.frame_duration_us,
        palette,
        lossy_downsample: options.lossy_downsample,
    };
    let frame_refs: Vec<&[u8]> = indexed_frames.iter().map(|v| v.as_slice()).collect();
    encode_av(
        &video_opts,
        &frame_refs,
        Some((&audio_opts, &samples_per_frame)),
        options.output_name.clone(),
        &mut out,
    )?;
    Ok(output_path)
}

/// Walk every pixel in every frame, building a colour-to-index map.
/// Returns the palette (256 entries, unused slots black) and one
/// index buffer per frame.
fn build_shared_palette(
    frames: &[image::RgbImage],
) -> Result<(Box<[[u8; 3]; 256]>, Vec<Vec<u8>>), FromAssetsError> {
    let mut map: HashMap<[u8; 3], u8> = HashMap::with_capacity(256);
    let mut palette = Box::new([[0u8; 3]; 256]);
    let mut indexed = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut buf = Vec::with_capacity(frame.width() as usize * frame.height() as usize);
        for px in frame.pixels() {
            let rgb = [px[0], px[1], px[2]];
            let idx = match map.get(&rgb) {
                Some(&i) => i,
                None => {
                    let next = map.len();
                    if next >= 256 {
                        return Err(FromAssetsError::TooManyColours);
                    }
                    palette[next] = rgb;
                    map.insert(rgb, next as u8);
                    next as u8
                }
            };
            buf.push(idx);
        }
        indexed.push(buf);
    }
    Ok((palette, indexed))
}

/// Split a contiguous interleaved sample buffer into `n_frames`
/// equal-sized chunks (the last chunk absorbs any leftover samples).
/// Each chunk is a multiple of `channels` so a stereo split never
/// straddles an L/R pair.
fn split_audio(samples: &[i16], n_frames: usize, channels: usize) -> Vec<Vec<i16>> {
    if n_frames == 0 {
        return Vec::new();
    }
    let total_frames_of_samples = samples.len() / channels;
    let per_video_frame = total_frames_of_samples / n_frames;
    let mut buckets = Vec::with_capacity(n_frames);
    let mut cursor = 0usize;
    for f in 0..n_frames {
        let frames_here = if f + 1 == n_frames {
            total_frames_of_samples - per_video_frame * (n_frames - 1)
        } else {
            per_video_frame
        };
        let len = frames_here * channels;
        buckets.push(samples[cursor..cursor + len].to_vec());
        cursor += len;
    }
    buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_audio_balances_correctly() {
        let s: Vec<i16> = (0..10).collect();
        let buckets = split_audio(&s, 3, 1);
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].len(), 3);
        assert_eq!(buckets[1].len(), 3);
        assert_eq!(buckets[2].len(), 4);
        let flat: Vec<i16> = buckets.into_iter().flatten().collect();
        assert_eq!(flat, s);
    }

    #[test]
    fn split_audio_keeps_stereo_pairs() {
        // 5 stereo frames = 10 samples; split across 3 video frames.
        let s: Vec<i16> = (0..10).collect();
        let buckets = split_audio(&s, 3, 2);
        for b in &buckets {
            assert_eq!(b.len() % 2, 0, "stereo split must keep L/R together");
        }
        let flat: Vec<i16> = buckets.into_iter().flatten().collect();
        assert_eq!(flat, s);
    }
}
