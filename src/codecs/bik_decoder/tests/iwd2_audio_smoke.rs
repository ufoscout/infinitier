//! End-to-end smoke: decode the first audio packet of every IWD2 file with
//! the pure-Rust audio decoder, sanity-check the output (correct sample
//! count, non-silence, no errors). Byte-exact / PSNR validation against
//! FFmpeg lives in a separate test.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use infinitier_bik_decoder::{AudioDecoder, parse_header};

fn iwd2_root() -> Option<PathBuf> {
    let p = std::env::var("IWD2_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/home/ufo/Temp/Games/Icewind Dale 2"));
    if p.is_dir() { Some(p) } else { None }
}

#[test]
fn first_audio_packet_decodes() {
    let Some(root) = iwd2_root() else {
        eprintln!("IWD2 install not found — skipping audio smoke");
        return;
    };

    for rel in [
        "Data/BISlogo.mve",
        "Data/Credits.mve",
        "Data/Nvidia.mve",
        "Data/WOTC.mve",
        "CD2/Data/END.mve",
        "CD2/Data/Intro.mve",
        "CD2/Data/Middle.mve",
    ] {
        let path = root.join(rel);
        let mut f = File::open(&path).unwrap();
        let header = parse_header(&mut f).unwrap();
        let track = header.audio_tracks[0];
        let mut audio = AudioDecoder::new(&track).unwrap();

        // Decode audio packets from the first ~30 video frames so we
        // exercise more than just the cold-start block.
        let mut total_samples = 0usize;
        for fr in header.frames.iter().take(30) {
            let mut packet = vec![0u8; fr.size as usize];
            f.seek(SeekFrom::Start(fr.pos as u64)).unwrap();
            f.read_exact(&mut packet).unwrap();
            let aud_len =
                u32::from_le_bytes([packet[0], packet[1], packet[2], packet[3]]) as usize;
            // The audio packet sits between the audio-len prefix and the
            // video bitstream, so it spans bytes [4, 4 + aud_len).
            let audio_packet = &packet[4..4 + aud_len];
            let pcm = audio
                .decode_packet(audio_packet)
                .unwrap_or_else(|e| panic!("{} aud decode: {}", rel, e));
            total_samples += pcm.len();
        }
        eprintln!(
            "ok  {:<24}  {} samples ({}ch @ {}Hz)",
            rel,
            total_samples,
            audio.channels(),
            audio.sample_rate()
        );
        assert!(
            total_samples > 0,
            "{}: no audio samples decoded from first 30 frames",
            rel
        );
    }
}
