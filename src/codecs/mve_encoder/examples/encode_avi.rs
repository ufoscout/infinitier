//! `encode_avi` — convert one or more AVI files to MVE.
//!
//! Pipeline (per input):
//!   1. `ffprobe` → width, height, frame rate
//!   2. Single-pass `ffmpeg` with split → palettegen → paletteuse →
//!      `-f rawvideo -pix_fmt pal8`, producing per-frame [1024 RGBA
//!      palette][W*H indices] records
//!   3. `infinitier_mve_encoder::encode_video` writes the .mve
//!
//! Usage:
//!   cargo run --example encode_avi -- <in.avi> <out.mve>
//!   cargo run --example encode_avi -- --batch <input_dir> <output_dir>

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use infinitier_mve_encoder::{encode_video, VideoOptions};

/// Per-frame palette header in `-f rawvideo -pix_fmt pal8` output:
/// 256 RGBA entries = 1024 bytes prepended before each frame's indices.
const PAL8_HEADER_BYTES: usize = 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--batch") if args.len() == 4 => {
            let input_dir = PathBuf::from(&args[2]);
            let output_dir = PathBuf::from(&args[3]);
            batch(&input_dir, &output_dir)
        }
        _ if args.len() == 3 => {
            let input = PathBuf::from(&args[1]);
            let output = PathBuf::from(&args[2]);
            encode_one(&input, &output)
        }
        _ => {
            eprintln!(
                "Usage:\n  {} <in.avi> <out.mve>\n  {} --batch <in_dir> <out_dir>",
                args[0], args[0]
            );
            std::process::exit(2);
        }
    }
}

fn batch(input_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let mut entries: Vec<PathBuf> = fs::read_dir(input_dir)?
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("avi"))
        .collect();
    entries.sort();
    if entries.is_empty() {
        return Err(format!("no .avi files in {}", input_dir.display()).into());
    }
    let mut failures = 0usize;
    for input in &entries {
        let stem = input.file_stem().unwrap().to_string_lossy().into_owned();
        let output = output_dir.join(format!("{stem}.mve"));
        match encode_one(input, &output) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("FAIL {}: {e}", input.display());
                failures += 1;
            }
        }
    }
    if failures > 0 {
        return Err(format!("{failures} of {} failed", entries.len()).into());
    }
    Ok(())
}

fn encode_one(input: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let (width, height, frame_dur_us) = ffprobe_video(input)?;
    if width % 8 != 0 || height % 8 != 0 {
        return Err(format!(
            "{}x{} not multiples of 8 — MVE encoder requires multiples of 8",
            width, height
        )
        .into());
    }

    let tmp = tempfile::tempdir()?;
    let frames_path = tmp.path().join("frames.raw");

    // Single-pass: split → palettegen → paletteuse → rawvideo pal8.
    // Output is `[1024 RGBA palette][W*H indices]` per frame.
    let filter = "split[a][b];\
        [a]palettegen=max_colors=256:reserve_transparent=0[p];\
        [b][p]paletteuse=dither=none";
    run_ffmpeg(&[
        "-y",
        "-i",
        input.to_str().ok_or("non-utf8 path")?,
        "-vf",
        filter,
        "-f",
        "rawvideo",
        "-pix_fmt",
        "pal8",
        frames_path.to_str().unwrap(),
    ])?;

    let raw = fs::read(&frames_path)?;
    let frame_pixels = width as usize * height as usize;
    let frame_stride = PAL8_HEADER_BYTES + frame_pixels;
    if raw.is_empty() || raw.len() % frame_stride != 0 {
        return Err(format!(
            "raw bytes {} not a multiple of per-frame stride {} (1024 RGBA palette + {} indices)",
            raw.len(),
            frame_stride,
            frame_pixels,
        )
        .into());
    }
    let n_frames = raw.len() / frame_stride;
    let palette = palette_from_pal8_header(&raw[..PAL8_HEADER_BYTES]);
    let frames: Vec<&[u8]> = (0..n_frames)
        .map(|i| {
            let start = i * frame_stride + PAL8_HEADER_BYTES;
            &raw[start..start + frame_pixels]
        })
        .collect();

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let label = output
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    // First pass: lossless. Retry with `lossy_downsample` if any
    // block's payload would have made the chunk overflow MVE's
    // 65,535-byte segment cap.
    let mut lossy = false;
    let mut buf = Vec::new();
    let opts = |lossy: bool| VideoOptions {
        width,
        height,
        frame_duration_us: frame_dur_us,
        palette: palette.clone(),
        lossy_downsample: lossy,
    };
    match encode_video(&opts(false), &frames, label.clone(), &mut buf) {
        Ok(()) => {}
        Err(infinitier_mve_encoder::MveEncodeError::ChunkTooBig) => {
            buf.clear();
            lossy = true;
            encode_video(&opts(true), &frames, label.clone(), &mut buf)?;
        }
        Err(e) => return Err(e.into()),
    }
    fs::write(output, &buf)?;

    let in_size = fs::metadata(input)?.len();
    let out_size = buf.len();
    println!(
        "{} -> {}  ({n_frames} frames @ {width}x{height}, {frame_dur_us} us/frame, {} -> {} bytes{})",
        input.display(),
        output.display(),
        in_size,
        out_size,
        if lossy { ", lossy 2x2 downsample fallback" } else { "" },
    );
    Ok(())
}

fn ffprobe_video(input: &Path) -> Result<(u16, u16, u32), Box<dyn Error>> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(input)
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "ffprobe failed for {}: {}",
            input.display(),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut frame_dur_us: Option<u32> = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("width=") {
            width = Some(v.trim().parse()?);
        } else if let Some(v) = line.strip_prefix("height=") {
            height = Some(v.trim().parse()?);
        } else if let Some(v) = line.strip_prefix("r_frame_rate=") {
            // e.g. "15/1" or "30000/1001"
            let v = v.trim();
            let (num, den) = v.split_once('/').unwrap_or((v, "1"));
            let num: f64 = num.parse()?;
            let den: f64 = den.parse()?;
            if num <= 0.0 || den <= 0.0 {
                return Err("invalid r_frame_rate".into());
            }
            frame_dur_us = Some((1_000_000.0 * den / num).round() as u32);
        }
    }
    Ok((
        width.ok_or("ffprobe missing width")? as u16,
        height.ok_or("ffprobe missing height")? as u16,
        frame_dur_us.ok_or("ffprobe missing r_frame_rate")?,
    ))
}

fn run_ffmpeg(args: &[&str]) -> Result<(), Box<dyn Error>> {
    let out = Command::new("ffmpeg").args(args).output()?;
    if !out.status.success() {
        return Err(format!(
            "ffmpeg failed: {}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(())
}

/// Decode the 1024-byte RGBA palette header that ffmpeg's `pal8`
/// rawvideo writes before each frame.
fn palette_from_pal8_header(header: &[u8]) -> Box<[[u8; 3]; 256]> {
    debug_assert_eq!(header.len(), PAL8_HEADER_BYTES);
    let mut palette = Box::new([[0u8; 3]; 256]);
    for (i, chunk) in header.chunks_exact(4).enumerate() {
        palette[i] = [chunk[0], chunk[1], chunk[2]];
    }
    palette
}
