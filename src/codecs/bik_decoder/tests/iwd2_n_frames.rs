//! Probe the decoder on the first N frames of one file. Phase-6 scaffold:
//! before generating SHA-256 fixtures across 12 000+ frames, make sure the
//! decoder gets past frames that exercise motion compensation, inter blocks,
//! and DCT residues. Frame 0 of every IWD2 cutscene is a flat fade-in, so
//! it's not a stress test on its own.

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

fn read_packet(file: &mut File, pos: u32, size: u32) -> Vec<u8> {
    let mut buf = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(pos as u64)).unwrap();
    file.read_exact(&mut buf).unwrap();
    buf
}

#[test]
fn decode_first_30_frames_of_bislogo() {
    let Some(root) = iwd2_root() else {
        eprintln!("IWD2 install not found — skipping");
        return;
    };
    let path = root.join("Data/BISlogo.mve");
    let mut f = File::open(&path).unwrap();
    let header = parse_header(&mut f).unwrap();
    let mut dec = VideoDecoder::new(&header).unwrap();

    for (i, fr) in header.frames.iter().take(30).enumerate() {
        let raw = read_packet(&mut f, fr.pos, fr.size);
        let aud_len = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        let video = &raw[4 + aud_len..];
        match dec.decode_frame(video) {
            Ok(frame) => {
                let y_avg =
                    frame.y.data.iter().map(|&b| b as u64).sum::<u64>() / frame.y.data.len() as u64;
                eprintln!("frame {:3}: ok  y_avg={}", i, y_avg);
            }
            Err(e) => panic!("frame {} decode failed: {}", i, e),
        }
    }
}
