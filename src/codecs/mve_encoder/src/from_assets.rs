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
    encode_av, encode_av_rgb555, pack_rgb555, palette_gen::quantise_to_palette8, AudioOptions,
    MveEncodeError, VideoOptions,
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
    /// When `true`, the encoder refuses to lose colour information
    /// during palette construction: if the input has > 256 unique
    /// colours it returns [`FromAssetsError::TooManyColours`]
    /// instead of falling back to median-cut quantisation. Default
    /// `false` — most users want auto-quantisation.
    ///
    /// Set this to `true` when the caller has pre-quantised the
    /// frames upstream (e.g. via `ffmpeg palettegen` +
    /// `paletteuse`) and wants to verify nothing further is being
    /// approximated.
    pub strict_palette: bool,
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
            .and_then(|r| r.with_guessed_format())
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

    // 2. Build a shared palette and per-frame index buffers. Falls
    //    back to median-cut quantisation when the input has > 256
    //    unique colours (unless `strict_palette = true`).
    let (palette, indexed_frames) = build_shared_palette(&rgb_frames, options.strict_palette)?;

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

/// HiColor (RGB555) variant of [`encode_from_assets`]: encodes a PNG
/// sequence + WAV audio into a 16-bit MVE. PNGs are read as RGB and
/// each pixel is packed into RGB555 via [`crate::pack_rgb555`] —
/// every channel is quantised from 8 to 5 bits (drop the low 3 bits),
/// so the round-trip is *visually* lossless and bit-exact at the
/// 5-bit-per-channel level. No palette generation is involved
/// (HiColor doesn't use an `OC_PALETTE` segment).
///
/// Unlike the Palette8 path, frames may contain **any** number of
/// distinct colours — there's no `TooManyColours` failure mode.
///
/// `wav_path` is optional. Pass `None` for a silent-track HiColor
/// MVE; pass `Some(path)` to mix in 16-bit PCM audio (mono or
/// stereo, sample rate ≤ 65535 Hz).
pub fn encode_from_assets_rgb555(
    png_paths: &[PathBuf],
    wav_path: Option<&Path>,
    options: &FromAssetsOptions,
    output_folder: &Path,
) -> Result<PathBuf, FromAssetsError> {
    if png_paths.is_empty() {
        return Err(FromAssetsError::NoFrames);
    }

    // 1. Read every PNG as RGB888 and convert to RGB555 u16 frames.
    let mut frames_u16: Vec<Vec<u16>> = Vec::with_capacity(png_paths.len());
    let mut size: Option<(u32, u32)> = None;
    for (i, path) in png_paths.iter().enumerate() {
        let img = ImageReader::open(path)
            .and_then(|r| r.with_guessed_format())
            .map_err(FromAssetsError::Io)?
            .decode()
            .map_err(|source| FromAssetsError::Image {
                path: path.clone(),
                source,
            })?
            .into_rgb8();
        let (w, h) = (img.width(), img.height());
        match size {
            None => size = Some((w, h)),
            Some((expected_w, expected_h)) => {
                if w != expected_w || h != expected_h {
                    return Err(FromAssetsError::FrameSizeMismatch {
                        index: i,
                        path: path.clone(),
                        got_w: w,
                        got_h: h,
                        exp_w: expected_w,
                        exp_h: expected_h,
                    });
                }
            }
        }
        let mut frame = Vec::with_capacity((w * h) as usize);
        for px in img.pixels() {
            frame.push(pack_rgb555(px[0], px[1], px[2]));
        }
        frames_u16.push(frame);
    }
    let (width_u32, height_u32) = size.expect("at least one frame");
    let width = width_u32 as u16;
    let height = height_u32 as u16;

    // 2. Read WAV samples if a path was provided.
    let mut audio_block: Option<(AudioOptions, Vec<Vec<i16>>)> = None;
    if let Some(wav_path) = wav_path {
        let mut reader =
            hound::WavReader::open(wav_path).map_err(|source| FromAssetsError::Wav {
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
        let aopts = AudioOptions {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        };
        let buckets = split_audio(&samples, frames_u16.len(), spec.channels as usize);
        audio_block = Some((aopts, buckets));
    }

    // 3. Encode.
    fs::create_dir_all(output_folder)?;
    let output_path = output_folder.join(format!("{}.mve", options.output_name));
    let mut out = fs::File::create(&output_path)?;

    let frame_refs: Vec<&[u16]> = frames_u16.iter().map(|v| v.as_slice()).collect();
    let audio_arg = audio_block
        .as_ref()
        .map(|(aopts, buckets)| (aopts, buckets.as_slice()));
    encode_av_rgb555(
        width,
        height,
        options.frame_duration_us,
        &frame_refs,
        options.lossy_downsample,
        audio_arg,
        options.output_name.clone(),
        &mut out,
    )?;
    Ok(output_path)
}

/// Build a 256-entry palette + per-frame index buffers from RGB888
/// `frames`. Uses the bit-exact fast-path when the input has ≤ 256
/// unique colours; otherwise falls back to median-cut quantisation
/// from [`crate::palette_gen`]. Returns `Err(TooManyColours)` only
/// when [`FromAssetsOptions::strict_palette`] is set and the
/// fast-path can't find a perfect mapping — useful when the caller
/// pre-quantised upstream and wants to be sure no further loss is
/// introduced here.
/// 256-entry RGB palette + one row-major index buffer per frame.
type SharedPalette = (Box<[[u8; 3]; 256]>, Vec<Vec<u8>>);

fn build_shared_palette(
    frames: &[image::RgbImage],
    strict_palette: bool,
) -> Result<SharedPalette, FromAssetsError> {
    // Try the fast-path first (bit-exact when ≤ 256 unique colours).
    let mut map: HashMap<[u8; 3], u8> = HashMap::with_capacity(256);
    let mut palette = Box::new([[0u8; 3]; 256]);
    let mut indexed: Vec<Vec<u8>> = Vec::with_capacity(frames.len());
    let mut over_budget = false;
    for frame in frames {
        let mut buf = Vec::with_capacity(frame.width() as usize * frame.height() as usize);
        for px in frame.pixels() {
            let rgb = [px[0], px[1], px[2]];
            let idx = match map.get(&rgb) {
                Some(&i) => i,
                None => {
                    let next = map.len();
                    if next >= 256 {
                        over_budget = true;
                        break;
                    }
                    palette[next] = rgb;
                    map.insert(rgb, next as u8);
                    next as u8
                }
            };
            buf.push(idx);
        }
        if over_budget {
            break;
        }
        indexed.push(buf);
    }
    if !over_budget {
        return Ok((palette, indexed));
    }
    if strict_palette {
        return Err(FromAssetsError::TooManyColours);
    }

    // Slow path: median-cut to 256 representatives.
    let frame_buffers: Vec<Vec<[u8; 3]>> = frames
        .iter()
        .map(|f| {
            f.pixels()
                .map(|px| [px[0], px[1], px[2]])
                .collect::<Vec<_>>()
        })
        .collect();
    let frame_refs: Vec<&[[u8; 3]]> = frame_buffers.iter().map(|f| f.as_slice()).collect();
    Ok(quantise_to_palette8(&frame_refs))
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
