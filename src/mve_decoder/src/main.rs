use mve_decoder::MveDecoder;

fn main() {
    let path = std::env::args().nth(1).expect("Usage: mve_decoder <file.mve>");
    let mut dec = MveDecoder::open(&path).expect("failed to open MVE file");
    let mut frame_count = 0u32;
    while let Some(frame) = dec.next_frame().expect("decode error") {
        frame_count += 1;
        let audio_samples: usize = frame.audio.iter().map(|a| a.samples.len()).sum();
        println!(
            "frame {frame_count:4}: {}x{}, {audio_samples} audio samples",
            frame.video.width, frame.video.height
        );
    }
    println!("Total frames: {frame_count}");
}
