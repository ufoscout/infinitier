use std::io::Cursor;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::DataSource;
use infinitier_test_utils::{get_assets_path, get_target_path};
use infinitier_wav_decoder::{WavDecoder, WavFormat};

/// Synthesise a tiny RIFF WAV in-memory.
fn synth_riff_wav(channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = WavWriter::new(&mut buf, spec).unwrap();
        for &s in samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }
    buf.into_inner()
}

/// Pull samples from a `WavDecoder` in small chunks to exercise the
/// streaming path. Stops on the first `Ok(0)` from `read_samples`.
fn drain_streaming(dec: &mut WavDecoder, chunk: usize) -> Vec<i16> {
    let mut out = Vec::new();
    let mut buf = vec![0i16; chunk];
    loop {
        let n = dec.read_samples(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    out
}

#[test]
fn streams_riff_wav_mono() {
    let pcm: Vec<i16> = (0..1024).map(|i| (i as i16).wrapping_mul(17)).collect();
    let bytes = synth_riff_wav(1, 22050, &pcm);

    let mut dec = WavDecoder::open(&DataSource::new(bytes)).unwrap();
    assert_eq!(dec.format(), WavFormat::Wav);
    let info = dec.info().clone();
    assert_eq!(info.channels, 1);
    assert_eq!(info.sample_rate, 22050);
    assert_eq!(info.bits_per_sample, 16);
    assert_eq!(info.total_values as usize, pcm.len());

    // Streamed read in tiny chunks, then a single decode_all after reset.
    let streamed = drain_streaming(&mut dec, 37);
    assert_eq!(streamed, pcm);

    dec.reset().unwrap();
    let bulk = dec.decode_all().unwrap();
    assert_eq!(bulk, pcm);
}

#[test]
fn streams_riff_wav_stereo() {
    let pcm: Vec<i16> = (0..2048)
        .map(|i| if i % 2 == 0 { i as i16 } else { -(i as i16) })
        .collect();
    let bytes = synth_riff_wav(2, 44100, &pcm);

    let mut dec = WavDecoder::open(&DataSource::new(bytes)).unwrap();
    assert_eq!(dec.info().channels, 2);
    assert_eq!(dec.info().sample_rate, 44100);

    let streamed = drain_streaming(&mut dec, 100);
    assert_eq!(streamed, pcm);
}

#[test]
fn rejects_non_16bit_riff() {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 24,
        sample_format: SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut w = WavWriter::new(&mut buf, spec).unwrap();
        for i in 0..16 {
            w.write_sample::<i32>(i * 1000).unwrap();
        }
        w.finalize().unwrap();
    }
    let err = WavDecoder::open(&DataSource::new(buf.into_inner())).unwrap_err();
    assert!(matches!(
        err,
        infinitier_wav_decoder::WavError::UnsupportedPcmFormat { bits: 24, .. }
    ));
}

/// Build a WAVC file by prepending the 28-byte WAVC header in front of an
/// existing ACM payload.
fn build_wavc(acm: &[u8], channels: u16, bits: u16, rate: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(28 + acm.len());
    out.extend_from_slice(b"WAVC");
    out.extend_from_slice(b"V1.0");
    out.extend_from_slice(&(acm.len() as u32).to_le_bytes()); // uncompressed size (placeholder)
    out.extend_from_slice(&(acm.len() as u32).to_le_bytes()); // compressed size
    out.extend_from_slice(&28u32.to_le_bytes());              // pointer to ACM data
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&0x777eu16.to_le_bytes());          // unused magic
    debug_assert_eq!(out.len(), 28);
    out.extend_from_slice(acm);
    out
}

fn ref_acm_samples(rel: &str) -> (Vec<i16>, u32, u32, u32) {
    let path = get_assets_path().join("resources/ACM").join(rel);
    let mut dec = AcmDecoder::open(&DataSource::new(path), OutputChannels::Original).unwrap();
    let info = dec.info.clone();
    let samples = dec.decode_all().unwrap();
    (samples, info.channels, info.rate, info.total_values)
}

#[test]
fn streams_wavc_built_from_acm_fixture() {
    let acm_rel = "bg/Bf1d1.ACM";
    let (ref_samples, channels, rate, total_values) = ref_acm_samples(acm_rel);

    let acm_bytes = std::fs::read(get_assets_path().join("resources/ACM").join(acm_rel)).unwrap();
    let wavc = build_wavc(&acm_bytes, channels as u16, 16, rate as u16);

    let mut dec = WavDecoder::open(&DataSource::new(wavc)).unwrap();
    assert_eq!(dec.format(), WavFormat::Wavc);
    let info = dec.info().clone();
    assert_eq!(info.channels, channels as u16);
    assert_eq!(info.sample_rate, rate);
    assert_eq!(info.bits_per_sample, 16);
    assert_eq!(info.total_values, total_values);

    // Stream in tiny chunks across block boundaries.
    let streamed = drain_streaming(&mut dec, 313);
    assert_eq!(streamed, ref_samples);

    // Reset → decode_all should match too.
    dec.reset().unwrap();
    let bulk = dec.decode_all().unwrap();
    assert_eq!(bulk, ref_samples);
}

#[test]
fn decode_to_file_round_trips() {
    let pcm: Vec<i16> = (0..8192).map(|i| (i as i16) ^ 0x55).collect();
    let bytes = synth_riff_wav(2, 22050, &pcm);

    let mut dec = WavDecoder::open(&DataSource::new(bytes)).unwrap();

    let out_path = get_target_path()
        .join("wav_decoder_test_output")
        .join("round_trip.wav");
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    dec.decode_to_file(&out_path).unwrap();

    let mut reader = WavReader::open(&out_path).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 2);
    assert_eq!(spec.sample_rate, 22050);
    assert_eq!(spec.bits_per_sample, 16);
    let read_back: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(read_back, pcm);
}

#[test]
fn rejects_unknown_magic() {
    let bytes = b"NOPE\x00\x00\x00\x00".to_vec();
    let err = WavDecoder::open(&DataSource::new(bytes)).unwrap_err();
    assert!(matches!(
        err,
        infinitier_wav_decoder::WavError::UnknownFormat(_)
    ));
}
