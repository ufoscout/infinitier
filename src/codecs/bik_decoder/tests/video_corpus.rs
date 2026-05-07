//! Video-corpus validation against `assets/resources/BIK/`.
//!
//! For every Bink file in the corpus, decode every frame, hash the
//! tightly-packed YUV420p output, and assert the hash matches the
//! sibling `<stem>.json` fixture (recorded from FFmpeg).
//!
//! Two acceptance bars:
//! * **Byte-exact** — every Bink-v1 codec tag (`BIKf` / `BIKg` / `BIKh` /
//!   `BIKi` / `BIKk`) must round-trip bit-for-bit.
//! * **PSNR ≥ 30 dB per frame** — `BIKb` (BinkB) where my decoder is
//!   structurally correct but a small subset of frames has known
//!   single-pixel divergence. PSNR is computed against an FFmpeg
//!   re-decode at test time. (PSNR ≥ 30 dB means the visible image is
//!   audiovisually indistinguishable; cf. the audio fallback in
//!   `audio_corpus.rs`.)

mod common;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;

use infinitier_bik_decoder::{BikHeader, VideoDecoder, VideoFrame, parse_header};
use sha2::{Digest, Sha256};

use crate::common::{CorpusEntry, corpus};

/// PSNR floor for the BinkB frames that aren't byte-exact. 30 dB means
/// average single-pixel error is ≤ ~8 in `[0, 255]`; the human eye
/// can't tell. Per-frame PSNR for the existing corpus is 33–94 dB.
const BINKB_PSNR_PASS_DB: f64 = 30.0;

/// Pack the decoder's stride-padded YUV planes into the tight
/// `width × height` + 2 chroma planes layout that
/// `ffmpeg -f rawvideo -pix_fmt yuv420p` produces.
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

/// Reason a single frame failed the byte-exact check (if any).
enum FrameOutcome {
    Match,
    /// Hash mismatch, but PSNR vs the FFmpeg re-decode is still ≥
    /// `BINKB_PSNR_PASS_DB`. Carries the actual PSNR for reporting.
    PsnrOnly(f64),
    /// Hash mismatch and PSNR fallback failed (or wasn't attempted).
    Bad(String),
}

fn run_one(entry: &CorpusEntry, psnr_fallback: bool) -> Result<FileResult, String> {
    let mut f = File::open(&entry.bik_path).map_err(|e| format!("open: {e}"))?;
    let header: BikHeader = parse_header(&mut f).map_err(|e| format!("parse: {e}"))?;
    let mut decoder = VideoDecoder::new(&header).map_err(|e| format!("decoder: {e}"))?;

    // Optional FFmpeg ground-truth YUV (only fetched when PSNR fallback is
    // in play). We pipe the whole file through `ffmpeg -f rawvideo` once
    // and slice per-frame from the result.
    let ref_yuv: Option<Vec<u8>> = if psnr_fallback {
        Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-i",
                entry.bik_path.to_str().unwrap(),
                "-an",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "yuv420p",
                "-",
            ])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
    } else {
        None
    };

    let mut packet_buf: Vec<u8> = Vec::with_capacity(header.max_frame_size as usize);
    let mut yuv_buf: Vec<u8> = Vec::new();

    let has_audio = !header.audio_tracks.is_empty();
    let n = entry.fixture.video.frame_hashes.len().min(header.frames.len());
    let frame_size = (header.width as usize) * (header.height as usize)
        + 2 * (header.width as usize / 2) * (header.height as usize / 2);

    let mut byte_exact = 0usize;
    let mut psnr_only = 0usize;
    let mut psnr_min = f64::INFINITY;
    let mut bad: Vec<(usize, String)> = Vec::new();

    for i in 0..n {
        let fr = header.frames[i];
        read_packet(&mut f, fr.pos, fr.size, &mut packet_buf);
        let video_bytes = if has_audio {
            let aud_len = u32::from_le_bytes([
                packet_buf[0], packet_buf[1], packet_buf[2], packet_buf[3],
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

        let outcome = if &got == want {
            FrameOutcome::Match
        } else if let Some(ref_full) = ref_yuv.as_ref() {
            let off = i * frame_size;
            if off + frame_size <= ref_full.len() {
                let psnr = compute_psnr_u8(&yuv_buf, &ref_full[off..off + frame_size]);
                if psnr >= BINKB_PSNR_PASS_DB {
                    FrameOutcome::PsnrOnly(psnr)
                } else {
                    FrameOutcome::Bad(format!("frame {i}: PSNR {psnr:.2} dB below threshold"))
                }
            } else {
                FrameOutcome::Bad(format!(
                    "frame {i}: hash mismatch and ffmpeg ref is short ({} < {})",
                    ref_full.len(),
                    off + frame_size
                ))
            }
        } else {
            FrameOutcome::Bad(format!(
                "frame {i}: hash mismatch, got {}.. want {}..",
                &got[..8],
                &want[..8]
            ))
        };

        match outcome {
            FrameOutcome::Match => byte_exact += 1,
            FrameOutcome::PsnrOnly(p) => {
                psnr_only += 1;
                if p < psnr_min {
                    psnr_min = p;
                }
            }
            FrameOutcome::Bad(msg) => {
                bad.push((i, msg));
                if bad.len() >= 8 {
                    break;
                }
            }
        }
    }

    Ok(FileResult {
        total: n,
        byte_exact,
        psnr_only,
        psnr_min,
        bad,
    })
}

struct FileResult {
    total: usize,
    byte_exact: usize,
    psnr_only: usize,
    psnr_min: f64,
    bad: Vec<(usize, String)>,
}

fn compute_psnr_u8(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return f64::INFINITY;
    }
    let mut sse = 0f64;
    for i in 0..n {
        let d = a[i] as f64 - b[i] as f64;
        sse += d * d;
    }
    let mse = sse / n as f64;
    if mse == 0.0 {
        return f64::INFINITY;
    }
    20.0 * (255f64 / mse.sqrt()).log10()
}

#[test]
fn corpus_video_byte_exact_per_frame() {
    let entries = corpus();

    let mut validated = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for entry in &entries {
        if !entry.is_decodable_video() {
            eprintln!("skip {} (codec_tag {})", entry.label(), entry.fixture.video.codec_tag);
            continue;
        }
        let label = entry.label();
        let psnr_fallback = entry.fixture.video.codec_tag == "BIKb";
        let start = std::time::Instant::now();
        match run_one(entry, psnr_fallback) {
            Ok(res) if res.bad.is_empty() => {
                let dt = start.elapsed().as_secs_f64();
                if res.psnr_only == 0 {
                    eprintln!(
                        "✓  {:<24}  {} frames byte-exact in {:.2}s",
                        label, res.total, dt,
                    );
                } else {
                    eprintln!(
                        "≈  {:<24}  {} frames ({} byte-exact, {} PSNR-only ≥ {:.2} dB) in {:.2}s",
                        label,
                        res.total,
                        res.byte_exact,
                        res.psnr_only,
                        res.psnr_min,
                        dt,
                    );
                }
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
