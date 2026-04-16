/// Decode all audio from an MVE file and write it to a WAV file.
/// Usage: mve_dump_audio <input.mve> <output.wav>
///
/// This lets you verify whether crackling is in the decoded samples
/// or in the rodio playback pipeline.
use hound::{SampleFormat, WavSpec, WavWriter};
use infinitier_mve_decoder::MveDecoder;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: mve_dump_audio <input.mve> <output.wav>");
        std::process::exit(1);
    }

    let mve_path = PathBuf::from(&args[1]);
    let wav_path = &args[2];

    let mut dec = MveDecoder::open(&mve_path).expect("failed to open MVE file");

    // Collect all audio samples first to detect format
    let mut all_samples: Vec<i16> = Vec::new();
    let mut channels = 1u16;
    let mut sample_rate = 22050u32;
    let mut frame_count = 0u32;

    loop {
        match dec.next_frame().expect("decode error") {
            Some(frame) => {
                frame_count += 1;
                for chunk in frame.audio {
                    if all_samples.is_empty() {
                        channels = chunk.channels as u16;
                        sample_rate = chunk.sample_rate;
                    }
                    all_samples.extend_from_slice(&chunk.samples);
                }
            }
            None => break,
        }
    }

    println!(
        "Decoded {frame_count} frames, {} total samples, {channels}ch @ {sample_rate}Hz",
        all_samples.len()
    );

    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(wav_path, spec).expect("failed to create WAV file");
    for &s in &all_samples {
        writer.write_sample(s).expect("write failed");
    }
    writer.finalize().expect("finalize failed");

    let duration_secs = all_samples.len() as f64 / (channels as f64 * sample_rate as f64);
    println!("Written to {wav_path} ({duration_secs:.2}s of audio)");
}
