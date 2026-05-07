//! Byte-exact first-frame comparison against FFmpeg-decoded reference YUV.
//!
//! The reference YUV files live at `/tmp/ff_<name>.yuv` and are produced by
//! `python3 /tmp/byte_compare.py` (which calls FFmpeg). When they're
//! missing, the test logs and skips — same pattern as `iwd2_container.rs`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use infinitier_bik_decoder::{VideoDecoder, parse_header};

fn iwd2_root() -> Option<PathBuf> {
    let p = std::env::var("IWD2_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/ufo/Temp/Games/Icewind Dale 2"));
    if p.is_dir() { Some(p) } else { None }
}

fn ref_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/ff_{}.yuv", name.replace('/', "_").replace(".mve", "")))
}

fn read_frame_packet(file: &mut File, pos: u32, size: u32) -> Vec<u8> {
    let mut buf = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(pos as u64)).unwrap();
    file.read_exact(&mut buf).unwrap();
    buf
}

#[test]
fn first_frame_matches_ffmpeg_byte_exact() {
    let Some(root) = iwd2_root() else {
        eprintln!("IWD2 install not found — skipping byte-exact test");
        return;
    };

    let files = [
        "Data/BISlogo.mve",
        "Data/Credits.mve",
        "Data/Nvidia.mve",
        "Data/WOTC.mve",
        "CD2/Data/END.mve",
        "CD2/Data/Intro.mve",
        "CD2/Data/Middle.mve",
    ];

    let mut any_skipped = false;
    let mut mismatches: Vec<String> = Vec::new();

    for rel in files {
        let ref_p = ref_path(rel);
        if !ref_p.exists() {
            eprintln!("(skip {} — reference {} missing)", rel, ref_p.display());
            any_skipped = true;
            continue;
        }
        let ref_bytes = std::fs::read(&ref_p).unwrap();

        let path = root.join(rel);
        let mut f = File::open(&path).unwrap();
        let header = parse_header(&mut f).unwrap_or_else(|e| panic!("{}: {}", rel, e));
        let mut decoder = VideoDecoder::new(&header).unwrap();

        let frame0 = header.frames[0];
        let raw = read_frame_packet(&mut f, frame0.pos, frame0.size);
        let aud_len = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        let video_bytes = &raw[4 + aud_len..];
        let frame = decoder
            .decode_frame(video_bytes)
            .unwrap_or_else(|e| panic!("decode {}: {}", rel, e));

        let w = header.width as usize;
        let h = header.height as usize;
        let cw = w / 2;
        let ch = h / 2;

        // Concatenate Y, U, V from the Rust decoder into a tightly-packed
        // YUV420p buffer (FFmpeg's reference doesn't pad).
        let mut packed: Vec<u8> = Vec::with_capacity(w * h + 2 * cw * ch);
        for row in 0..h {
            packed.extend_from_slice(&frame.y.data[row * frame.y.stride..row * frame.y.stride + w]);
        }
        for row in 0..ch {
            packed.extend_from_slice(&frame.u.data[row * frame.u.stride..row * frame.u.stride + cw]);
        }
        for row in 0..ch {
            packed.extend_from_slice(&frame.v.data[row * frame.v.stride..row * frame.v.stride + cw]);
        }

        // Compare against the first frame's worth of reference bytes.
        let cmp_len = packed.len().min(ref_bytes.len());
        let diffs = packed[..cmp_len]
            .iter()
            .zip(&ref_bytes[..cmp_len])
            .filter(|(a, b)| a != b)
            .count();

        let total = cmp_len;
        if diffs == 0 {
            eprintln!("✓  {:<24}  byte-exact ({} bytes)", rel, total);
        } else {
            // Report the first 5 differences for diagnostics.
            let first_diffs: Vec<(usize, u8, u8)> = packed[..cmp_len]
                .iter()
                .zip(&ref_bytes[..cmp_len])
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(5)
                .map(|(i, (&a, &b))| (i, a, b))
                .collect();
            mismatches.push(format!(
                "✗  {:<24}  {} / {} bytes differ ({:.4}%); first diffs: {:?}",
                rel,
                diffs,
                total,
                100.0 * diffs as f64 / total as f64,
                first_diffs
            ));
        }
    }

    if !mismatches.is_empty() {
        eprintln!();
        for m in &mismatches {
            eprintln!("{}", m);
        }
        panic!(
            "{}/{} files do not match FFmpeg byte-exactly",
            mismatches.len(),
            files.len()
        );
    }
    if any_skipped {
        eprintln!("(some references were missing — re-run /tmp/byte_compare.py to regenerate)");
    }
}
