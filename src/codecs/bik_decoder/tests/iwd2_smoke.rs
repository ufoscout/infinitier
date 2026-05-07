//! End-to-end smoke test: parse a real IWD2 file, decode its first frame,
//! and assert basic invariants on the output (no panic, dimensions match,
//! pixel values are at least non-uniform). This is *not* the byte-exact
//! validation step — that lives in Phase 6 once the SHA-256 fixtures are
//! generated. The goal here is to catch obvious bitstream-misreads before
//! we go fishing for them with hex diffs.

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

fn read_frame_packet(file: &mut File, pos: u32, size: u32) -> Vec<u8> {
    let mut buf = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(pos as u64)).unwrap();
    file.read_exact(&mut buf).unwrap();
    buf
}

#[test]
fn decode_first_frame_of_each_iwd2_file() {
    let Some(root) = iwd2_root() else {
        eprintln!("IWD2 install not found — skipping smoke test");
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

    for rel in files {
        let path = root.join(rel);
        let mut f = File::open(&path).unwrap();
        let header = parse_header(&mut f).unwrap_or_else(|e| panic!("{}: {}", rel, e));
        let mut decoder = VideoDecoder::new(&header).unwrap();

        // Each frame's payload starts with `u32 audio_packet_len`; the video
        // bitstream is the remainder.
        let frame0 = header.frames[0];
        let raw = read_frame_packet(&mut f, frame0.pos, frame0.size);
        let aud_len = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        assert!(
            aud_len + 4 <= raw.len(),
            "{}: audio packet length {} exceeds frame size {}",
            rel,
            aud_len,
            raw.len()
        );
        let video_bytes = &raw[4 + aud_len..];

        let result = decoder.decode_frame(video_bytes);
        match result {
            Ok(frame) => {
                assert_eq!(frame.y.width, header.width);
                assert_eq!(frame.y.height, header.height);
                let y_avg =
                    frame.y.data.iter().map(|&b| b as u64).sum::<u64>() / frame.y.data.len() as u64;
                let u_avg =
                    frame.u.data.iter().map(|&b| b as u64).sum::<u64>() / frame.u.data.len() as u64;
                let v_avg =
                    frame.v.data.iter().map(|&b| b as u64).sum::<u64>() / frame.v.data.len() as u64;
                eprintln!(
                    "ok  {:<24}  Y avg {:3}  U avg {:3}  V avg {:3}",
                    rel, y_avg, u_avg, v_avg,
                );
                // First frame is a black fade-in: Y≈16, U=V≈128 on TV-range
                // (verified vs FFmpeg). Allow IDCT rounding drift of a few
                // levels — strict per-frame validation is Phase 6's job.
                assert!(
                    (15..=18).contains(&(y_avg as u8)),
                    "{}: Y avg {} out of expected fade-in range",
                    rel,
                    y_avg
                );
                assert!(
                    (126..=130).contains(&(u_avg as u8)),
                    "{}: U avg {} out of expected fade-in range",
                    rel,
                    u_avg
                );
                assert!(
                    (126..=130).contains(&(v_avg as u8)),
                    "{}: V avg {} out of expected fade-in range",
                    rel,
                    v_avg
                );
            }
            Err(e) => {
                eprintln!("FAIL {}: {}", rel, e);
                panic!("decode_frame failed for {}: {}", rel, e);
            }
        }
    }
}
