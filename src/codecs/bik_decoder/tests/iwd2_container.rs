//! Container-parser corpus test against the seven IWD2 cutscene files.
//!
//! Each file ships with `.mve` extension but is actually Bink Video v1
//! (`BIKi`). We parse the header, frame index, and audio track table, and
//! check the expected dimensions / FPS / audio params against ground-truth
//! values previously captured from FFmpeg's `ffprobe`.
//!
//! When the IWD2 install isn't present (typical for CI), the test logs and
//! exits — there's no canonical place to ship the binaries in-repo.

use std::path::{Path, PathBuf};

use infinitier_bik_decoder::{AudioFlags, parse_header};

/// Override the IWD2 install path with the env var if you have it elsewhere.
fn iwd2_root() -> Option<PathBuf> {
    let p = std::env::var("IWD2_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/ufo/Temp/Games/Icewind Dale 2"));
    if p.is_dir() { Some(p) } else { None }
}

/// Ground-truth metadata from `ffprobe` (taken on 2026-05-07).
/// `(relative_path, frame_count_min, width, height, fps_num, fps_den,
///   sample_rate, channels, dct_audio)`.
struct Expected {
    rel: &'static str,
    width: u32,
    height: u32,
    fps_num: u32,
    fps_den: u32,
    sample_rate: u16,
    channels: u16,
    dct_audio: bool,
}

const CORPUS: &[Expected] = &[
    Expected {
        rel: "Data/BISlogo.mve",
        width: 640,
        height: 320,
        fps_num: 15,
        fps_den: 1,
        sample_rate: 22050,
        channels: 2,
        dct_audio: true,
    },
    Expected {
        rel: "Data/Credits.mve",
        width: 640,
        height: 320,
        fps_num: 15,
        fps_den: 1,
        sample_rate: 22050,
        channels: 2,
        dct_audio: true,
    },
    Expected {
        rel: "Data/Nvidia.mve",
        width: 640,
        height: 480,
        fps_num: 6025,
        fps_den: 201,
        sample_rate: 48000,
        channels: 2,
        dct_audio: true,
    },
    Expected {
        rel: "Data/WOTC.mve",
        width: 640,
        height: 320,
        fps_num: 15,
        fps_den: 1,
        sample_rate: 22050,
        channels: 2,
        dct_audio: true,
    },
    Expected {
        rel: "CD2/Data/END.mve",
        width: 640,
        height: 480,
        fps_num: 2997,
        fps_den: 100,
        sample_rate: 22050,
        channels: 2,
        dct_audio: true,
    },
    Expected {
        rel: "CD2/Data/Intro.mve",
        width: 640,
        height: 480,
        fps_num: 2997,
        fps_den: 100,
        sample_rate: 22050,
        channels: 2,
        dct_audio: true,
    },
    Expected {
        rel: "CD2/Data/Middle.mve",
        width: 640,
        height: 480,
        fps_num: 2997,
        fps_den: 100,
        sample_rate: 44100,
        channels: 2,
        dct_audio: true,
    },
];

#[test]
fn iwd2_container_parses_match_ffprobe() {
    let Some(root) = iwd2_root() else {
        eprintln!("IWD2 install not found — skipping iwd2_container test");
        return;
    };

    for exp in CORPUS {
        let path: PathBuf = root.join(exp.rel);
        let mut file =
            std::fs::File::open(&path).unwrap_or_else(|e| panic!("open {}: {}", path.display(), e));
        let h = parse_header(&mut file)
            .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

        assert_eq!(&h.signature, b"BIKi", "{}: signature", exp.rel);
        assert_eq!(h.width, exp.width, "{}: width", exp.rel);
        assert_eq!(h.height, exp.height, "{}: height", exp.rel);
        assert_eq!(h.fps_num, exp.fps_num, "{}: fps_num", exp.rel);
        assert_eq!(h.fps_den, exp.fps_den, "{}: fps_den", exp.rel);
        assert_eq!(h.frames.len() as u32, h.frame_count);

        // File-size sanity: the on-disk size should match the header's
        // file_size (within ±8 bytes for trailing pad — gemrb adds 8, which
        // is what we already do in `parse_header`).
        let on_disk = file.metadata().unwrap().len();
        assert_eq!(
            h.file_size,
            on_disk,
            "{}: file_size header vs disk ({} vs {})",
            exp.rel,
            h.file_size,
            on_disk
        );

        assert_eq!(h.audio_tracks.len(), 1, "{}: audio_tracks count", exp.rel);
        let t = h.audio_tracks[0];
        assert_eq!(t.sample_rate, exp.sample_rate, "{}: sample_rate", exp.rel);
        assert_eq!(t.flags.channels(), exp.channels, "{}: channels", exp.rel);
        assert_eq!(
            t.flags.contains(AudioFlags::USE_DCT),
            exp.dct_audio,
            "{}: USE_DCT",
            exp.rel
        );

        // Frame index sanity: each frame's range fits inside the file.
        for (i, fr) in h.frames.iter().enumerate() {
            assert!(
                fr.pos as u64 + fr.size as u64 <= h.file_size,
                "{}: frame {} extends past EOF",
                exp.rel,
                i
            );
            assert!(fr.size > 0, "{}: frame {} has zero size", exp.rel, i);
        }
        // First frame is always a keyframe.
        assert!(h.frames[0].keyframe, "{}: first frame not keyframe", exp.rel);

        eprintln!(
            "ok  {:<24}  {}x{} @ {:.2}fps   {} frames, {} audio @ {} Hz {}ch",
            exp.rel,
            h.width,
            h.height,
            h.fps(),
            h.frame_count,
            if t.flags.contains(AudioFlags::USE_DCT) { "DCT" } else { "RDFT" },
            t.sample_rate,
            t.flags.channels(),
        );
    }
}

#[test]
fn rejects_non_bink_signature() {
    let buf: &[u8] = b"BOGUS\0\0\0";
    let mut cur = std::io::Cursor::new(buf);
    let err = parse_header(&mut cur).unwrap_err();
    assert!(matches!(
        err,
        infinitier_bik_decoder::BikError::BadSignature(_)
    ));
}

// Helper for ad-hoc poking; left enabled so `cargo test` reports it once.
#[allow(dead_code)]
fn dummy_path_check(_: &Path) {}
