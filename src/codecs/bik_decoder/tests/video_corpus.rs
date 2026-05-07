//! Video-corpus validation against `assets/resources/BIK/`.
//!
//! For every Bink file in the corpus, decode every frame, hash the
//! tightly-packed YUV420p output, and assert the hash matches the
//! sibling `<stem>.json` fixture (recorded from this crate's decoder
//! via `cargo run --example gen_fixtures`).

mod common;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use infinitier_bik_decoder::{BikHeader, VideoDecoder, VideoFrame, parse_header};
use sha2::{Digest, Sha256};

use crate::common::{CorpusEntry, corpus};

/// Pack the decoder's stride-padded YUV planes into the tight
/// `width × height` + 2 chroma planes layout that the fixture
/// generator hashes.
fn pack_yuv420p(frame: &VideoFrame, w: u32, h: u32, out: &mut Vec<u8>) {
    let w = w as usize;
    let h = h as usize;
    let cw = w / 2;
    let ch = h / 2;
    out.clear();
    out.reserve(w * h + 2 * cw * ch);
    for row in 0..h {
        out.extend_from_slice(&frame.y.data[row * frame.y.stride..row * frame.y.stride + w]);
    }
    for row in 0..ch {
        out.extend_from_slice(&frame.u.data[row * frame.u.stride..row * frame.u.stride + cw]);
    }
    for row in 0..ch {
        out.extend_from_slice(&frame.v.data[row * frame.v.stride..row * frame.v.stride + cw]);
    }
}

fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_packet(file: &mut File, pos: u32, size: u32, into: &mut Vec<u8>) {
    into.resize(size as usize, 0);
    file.seek(SeekFrom::Start(pos as u64)).unwrap();
    file.read_exact(into).unwrap();
}

struct FileResult {
    total: usize,
    bad: Vec<(usize, String)>,
}

fn run_one(entry: &CorpusEntry) -> Result<FileResult, String> {
    let mut f = File::open(&entry.bik_path).map_err(|e| format!("open: {e}"))?;
    let header: BikHeader = parse_header(&mut f).map_err(|e| format!("parse: {e}"))?;
    let mut decoder = VideoDecoder::new(&header).map_err(|e| format!("decoder: {e}"))?;

    let mut packet_buf: Vec<u8> = Vec::with_capacity(header.max_frame_size as usize);
    let mut yuv_buf: Vec<u8> = Vec::new();

    let has_audio = !header.audio_tracks.is_empty();
    let n = entry.fixture.video.frame_hashes.len().min(header.frames.len());

    let mut bad: Vec<(usize, String)> = Vec::new();

    for i in 0..n {
        let fr = header.frames[i];
        read_packet(&mut f, fr.pos, fr.size, &mut packet_buf);
        let video_bytes = if has_audio {
            let aud_len = u32::from_le_bytes([
                packet_buf[0],
                packet_buf[1],
                packet_buf[2],
                packet_buf[3],
            ]) as usize;
            &packet_buf[4 + aud_len..]
        } else {
            &packet_buf[..]
        };
        let frame = decoder
            .decode_frame(video_bytes)
            .map_err(|e| format!("frame {i}: {e}"))?;
        pack_yuv420p(frame, header.width, header.height, &mut yuv_buf);
        let got = hash_hex(&yuv_buf);
        let want = &entry.fixture.video.frame_hashes[i];
        if &got != want {
            bad.push((
                i,
                format!(
                    "frame {i}: hash mismatch, got {}.. want {}..",
                    &got[..8],
                    &want[..8]
                ),
            ));
            if bad.len() >= 8 {
                break;
            }
        }
    }

    Ok(FileResult { total: n, bad })
}

#[test]
fn corpus_video_byte_exact_per_frame() {
    let entries = corpus();

    let mut validated = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for entry in &entries {
        if !entry.is_decodable_video() {
            eprintln!(
                "skip {} (codec_tag {})",
                entry.label(),
                entry.fixture.video.codec_tag
            );
            continue;
        }
        let label = entry.label();
        let start = std::time::Instant::now();
        match run_one(entry) {
            Ok(res) if res.bad.is_empty() => {
                let dt = start.elapsed().as_secs_f64();
                eprintln!(
                    "✓  {:<24}  {} frames byte-exact in {:.2}s",
                    label, res.total, dt,
                );
                validated += 1;
            }
            Ok(res) => {
                let first = &res.bad[0];
                eprintln!(
                    "✗  {:<24}  {} bad frames (first {}: {})",
                    label,
                    res.bad.len(),
                    first.0,
                    first.1
                );
                failed.push(label.to_owned());
            }
            Err(e) => {
                eprintln!("!! {}: {}", label, e);
                failed.push(label.to_owned());
            }
        }
    }

    eprintln!(
        "\nsummary: {} files validated, {} failed",
        validated,
        failed.len()
    );
    assert!(validated > 0, "no decodable Bink files in corpus");
    assert!(failed.is_empty(), "files failed: {:?}", failed);
}
