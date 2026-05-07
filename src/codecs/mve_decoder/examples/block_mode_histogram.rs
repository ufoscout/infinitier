//! Decode every video frame in an MVE file and report how often each
//! 8×8 block-coding opcode (0x0..=0xF) occurs. Useful for reverse-
//! engineering an encoder's behaviour: feed it different inputs and
//! see which modes it picks.
//!
//! Usage:
//!     cargo run --example block_mode_histogram -- path/to/file.mve [more.mve ...]

use std::env;
use std::path::{Path, PathBuf};

use infinitier_datasource::{DataSource, DataTrait};
use infinitier_mve_decoder::{BlockModeStats, MveDecoder, VideoFormat};

/// Hand-labelled mode names from the decoder source — keep the
/// indexing in sync with `video.rs::decode_frame8` if those ever
/// change.
const MODE_NAMES_8: [&str; 16] = [
    "0x0 copy_prev_block",     // direct copy from buf2 at same pos
    "0x1 keep_2_frames_back",  // skip — pixels already from 2 frames ago
    "0x2 cur_frame_motion_lo", // current-frame MV, low range
    "0x3 cur_frame_motion_hi", // current-frame MV, full range
    "0x4 prev_frame_motion_a", // previous-frame MV (style A)
    "0x5 prev_frame_motion_b", // previous-frame MV (style B)
    "0x6 (reserved)",
    "0x7 delta_pattern",
    "0x8 quad_a",
    "0x9 quad_b",
    "0xa quad_c",
    "0xb quad_d",
    "0xc 4x4_fill",
    "0xd 8x4_fill",
    "0xe solid_colour",
    "0xf raw_pixels",
];

fn open_decoder(path: &Path) -> Result<MveDecoder<Box<dyn DataTrait>>, Box<dyn std::error::Error>> {
    let ds = DataSource::new(PathBuf::from(path));
    let reader = ds.reader()?;
    Ok(MveDecoder::new(reader, path.display().to_string())?)
}

fn drain_decoder<R: std::io::BufRead + std::io::Seek>(
    dec: &mut MveDecoder<R>,
) -> Result<VideoFormat, Box<dyn std::error::Error>> {
    let format = dec.format();
    while dec.next_frame()?.is_some() {}
    Ok(format)
}

fn print_histogram(label: &str, format: VideoFormat, stats: &BlockModeStats) {
    let total = stats.blocks.max(1) as f64;
    let counts = if format == VideoFormat::Palette8 {
        &stats.video8
    } else {
        &stats.video16
    };
    println!(
        "\n=== {label} ({format:?}, {} frames, {} blocks) ===",
        stats.frames, stats.blocks
    );
    println!("  {:<28} {:>10} {:>8}", "mode", "count", "share");
    for (i, &c) in counts.iter().enumerate() {
        if c == 0 {
            continue;
        }
        println!(
            "  {:<28} {:>10} {:>7.2}%",
            MODE_NAMES_8[i],
            c,
            (c as f64 / total) * 100.0
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: block_mode_histogram <file.mve> [more.mve ...]");
        std::process::exit(2);
    }

    for p in paths {
        let path = Path::new(&p);
        let mut dec = open_decoder(path)?;
        let format = drain_decoder(&mut dec)?;
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());
        print_histogram(&label, format, dec.block_mode_stats());
    }
    Ok(())
}
