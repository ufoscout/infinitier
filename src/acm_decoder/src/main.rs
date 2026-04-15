use std::{
    fs::File,
    path::{Path, PathBuf},
};

use clap::Parser;

use infinitier_acm_decoder::AcmDecoder;

/// ACM audio decoder — converts Interplay ACM files to WAV.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Input ACM file
    input: PathBuf,

    /// Output WAV file (defaults to <input>.wav)
    output: Option<PathBuf>,

    /// Force mono output (1 channel)
    #[arg(short = 'm', long, conflicts_with = "force_stereo")]
    force_mono: bool,

    /// Force stereo output (2 channels)
    #[arg(short = 's', long, conflicts_with = "force_mono")]
    force_stereo: bool,
}

fn main() {
    let args = Args::parse();

    let force_chans = if args.force_mono {
        1
    } else if args.force_stereo {
        2
    } else {
        0 // trust the file header
    };

    let input = &args.input;
    let output = args.output.unwrap_or_else(|| with_wav_extension(input));

    let f = File::open(input).unwrap_or_else(|e| {
        eprintln!("error: cannot open '{}': {e}", input.display());
        std::process::exit(1);
    });

    let mut dec = AcmDecoder::open(f, force_chans).unwrap_or_else(|e| {
        eprintln!("error: '{}': {e}", input.display());
        std::process::exit(1);
    });

    eprintln!(
        "{}: {} Hz, {} ch, {} samples",
        input.display(),
        dec.info.rate,
        dec.info.channels,
        dec.total_values / dec.info.channels,
    );

    let samples = dec.decode_all().unwrap_or_else(|e| {
        eprintln!("error: decoding '{}': {e}", input.display());
        std::process::exit(1);
    });

    let spec = hound::WavSpec {
        channels: dec.info.channels as u16,
        sample_rate: dec.info.rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&output, spec).unwrap_or_else(|e| {
        eprintln!("error: cannot create '{}': {e}", output.display());
        std::process::exit(1);
    });

    for &s in &samples {
        writer.write_sample(s).unwrap_or_else(|e| {
            eprintln!("error: writing WAV: {e}");
            std::process::exit(1);
        });
    }

    writer.finalize().unwrap_or_else(|e| {
        eprintln!("error: finalising WAV: {e}");
        std::process::exit(1);
    });

    eprintln!("wrote '{}'", output.display());
}

fn with_wav_extension(p: &Path) -> PathBuf {
    let mut out = p.to_path_buf();
    out.set_extension("wav");
    out
}
