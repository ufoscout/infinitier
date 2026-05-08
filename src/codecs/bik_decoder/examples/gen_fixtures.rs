//! Regenerate the JSON fixtures next to every `.bik` / `.mve` file under
//! `assets/resources/BIK/`. Each fixture records:
//!
//! * `video.codec_tag`, dimensions, frame count, frame duration, and the
//!   per-frame SHA-256 of the tightly-packed YUV420p output.
//! * `audio.channels`, `sample_rate`, `bits_per_sample`, `format`,
//!   `total_samples`, and the SHA-256 of the full interleaved s16le PCM.
//!
//! Both `audio_corpus.rs` and `video_corpus.rs` consume these JSONs as
//! self-consistency checks against this crate's pure-Rust decoder. Run
//! after any decoder-output-changing patch:
//!
//! ```sh
//! cargo run -p infinitier_bik_decoder --example gen_fixtures --release
//! ```

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use infinitier_bik_decoder::{
    AudioDecoder, AudioTrack, BikHeader, VideoDecoder, VideoFrame, parse_header,
};
use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path};
use serde::Serialize;
use sha2::{Digest, Sha256};

const BIK_FOLDER: &str = "resources/BIK";

#[derive(Serialize)]
struct CorpusFixture {
    video: VideoFixture,
    audio: Option<AudioFixture>,
}

#[derive(Serialize)]
struct VideoFixture {
    codec_tag: String,
    width: u32,
    height: u32,
    frame_count: u32,
    frame_duration_us: u64,
    frame_hashes: Vec<String>,
}

#[derive(Serialize)]
struct AudioFixture {
    channels: u32,
    sample_rate: u32,
    bits_per_sample: u32,
    format: String,
    total_samples: u64,
    wav_sha256: String,
}

fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

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

fn audio_fixture(file: &mut File, header: &BikHeader, track: &AudioTrack) -> AudioFixture {
    let channels = track.flags.channels() as u32;
    let sample_rate = track.sample_rate as u32;
    let mut audio = AudioDecoder::new(track).expect("audio init");

    let mut hasher = Sha256::new();
    let mut total_samples: u64 = 0;
    let mut packet = Vec::with_capacity(header.max_frame_size as usize);
    for fr in &header.frames {
        packet.resize(fr.size as usize, 0);
        file.seek(SeekFrom::Start(fr.pos as u64)).expect("seek");
        file.read_exact(&mut packet).expect("read");
        let aud_len = u32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]) as usize;
        let chunk = audio
            .decode_packet(&packet[4..4 + aud_len])
            .expect("audio decode");
        // `total_samples` records the interleaved sample count (i.e. one
        // unit per i16 emitted), matching the original fixture's
        // accounting where stereo doubles the figure relative to per-
        // channel frames.
        total_samples += chunk.len() as u64;
        for s in chunk {
            hasher.update(s.to_le_bytes());
        }
    }
    let digest: [u8; 32] = hasher.finalize().into();
    let stereo_label = if channels == 2 { "stereo" } else { "mono" };
    AudioFixture {
        channels,
        sample_rate,
        bits_per_sample: 16,
        format: format!("PCM 16-bit {stereo_label} at {sample_rate} Hz"),
        total_samples,
        wav_sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
    }
}

fn video_fixture(file: &mut File, header: &BikHeader) -> VideoFixture {
    let mut decoder = VideoDecoder::new(header).expect("video decoder");
    let mut packet_buf: Vec<u8> = Vec::with_capacity(header.max_frame_size as usize);
    let mut yuv_buf: Vec<u8> = Vec::new();
    let has_audio = !header.audio_tracks.is_empty();
    let mut hashes: Vec<String> = Vec::with_capacity(header.frames.len());

    for fr in &header.frames {
        packet_buf.resize(fr.size as usize, 0);
        file.seek(SeekFrom::Start(fr.pos as u64)).expect("seek");
        file.read_exact(&mut packet_buf).expect("read");
        let video_bytes = if has_audio {
            let aud_len =
                u32::from_le_bytes([packet_buf[0], packet_buf[1], packet_buf[2], packet_buf[3]])
                    as usize;
            &packet_buf[4 + aud_len..]
        } else {
            &packet_buf[..]
        };
        let frame = decoder.decode_frame(video_bytes).expect("decode frame");
        pack_yuv420p(frame, header.width, header.height, &mut yuv_buf);
        hashes.push(hash_hex(&yuv_buf));
    }

    let codec_tag = std::str::from_utf8(&header.signature)
        .expect("signature is ASCII")
        .to_owned();
    let frame_duration_us = header.fps_den as u64 * 1_000_000 / header.fps_num as u64;

    VideoFixture {
        codec_tag,
        width: header.width,
        height: header.height,
        frame_count: header.frame_count,
        frame_duration_us,
        frame_hashes: hashes,
    }
}

fn process(path: &Path) {
    let label = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
    let mut f = File::open(path).expect("open");
    let header = parse_header(&mut f).expect("parse header");

    let video = video_fixture(&mut f, &header);
    let audio = header
        .audio_tracks
        .first()
        .map(|track| audio_fixture(&mut f, &header, track));

    let fixture = CorpusFixture { video, audio };
    let json = serde_json::to_string_pretty(&fixture).expect("serialize");

    let json_path = path.with_extension("json");
    std::fs::write(&json_path, json + "\n").expect("write");
    eprintln!("wrote {} ({} frames)", json_path.display(), header.frame_count);
    let _ = label;
}

fn main() {
    let folder = get_assets_path().join(BIK_FOLDER);
    assert!(folder.is_dir(), "missing folder {}", folder.display());

    let mut paths = Vec::new();
    paths.extend(get_all_in_folder_by_extension(&folder, "bik"));
    paths.extend(get_all_in_folder_by_extension(&folder, "mve"));
    paths.sort();
    assert!(!paths.is_empty(), "no .bik / .mve files in {}", folder.display());

    for p in &paths {
        process(p);
    }
}
