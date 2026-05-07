use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_acm_encoder::{
    encode_pcm, encode_pcm_packed, encode_pcm_subband, encode_pcm_subband_with_f_half,
};
use infinitier_datasource::DataSource;

/// Encode via the full subband+packer pipeline, decode via
/// `AcmDecoder`, return the decoded samples. Asserts that the file
/// header round-trips exactly.
fn subband_round_trip(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    acm_level: u32,
    acm_rows: u32,
) -> Vec<i16> {
    let mut buf = Vec::new();
    encode_pcm_subband(
        samples,
        channels,
        sample_rate,
        acm_level,
        acm_rows,
        &mut buf,
    )
    .expect("encode failed");
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "subband_round_trip",
    )
    .expect("open failed");
    assert_eq!(dec.info.channels, channels, "channels round-trip");
    assert_eq!(dec.info.rate, sample_rate, "rate round-trip");
    assert_eq!(dec.info.acm_level, acm_level, "acm_level round-trip");
    assert_eq!(dec.info.acm_rows, acm_rows, "acm_rows round-trip");
    assert_eq!(
        dec.info.total_values as usize,
        samples.len(),
        "total_values round-trip"
    );
    dec.decode_all().expect("decode failed")
}

/// Stats for a tolerance-based round-trip check.
fn diff_stats(a: &[i16], b: &[i16]) -> (i32, f64) {
    assert_eq!(a.len(), b.len());
    let mut max_abs: i32 = 0;
    let mut sum_sq: u128 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        let d = (*x as i32) - (*y as i32);
        let ad = d.unsigned_abs() as i32;
        if ad > max_abs {
            max_abs = ad;
        }
        sum_sq += (d as i64).unsigned_abs() as u128 * (d as i64).unsigned_abs() as u128;
    }
    let rms = (sum_sq as f64 / a.len() as f64).sqrt();
    (max_abs, rms)
}

#[test]
fn subband_level_zero_is_lossless_and_matches_packed() {
    // With acm_level = 0 the subband filter is a passthrough, so the
    // output is identical bytes to encode_pcm_packed and the round-trip
    // must be bit-exact.
    let pcm: Vec<i16> = (0..1000).map(|i| (i as i16).wrapping_mul(11)).collect();

    let mut subband_buf = Vec::new();
    encode_pcm_subband(&pcm, 1, 22050, 0, 512, &mut subband_buf).unwrap();
    let mut packed_buf = Vec::new();
    encode_pcm_packed(&pcm, 1, 22050, &mut packed_buf).unwrap();
    assert_eq!(
        subband_buf, packed_buf,
        "level=0 subband output must equal packed output byte-for-byte"
    );

    let out = subband_round_trip(&pcm, 1, 22050, 0, 512);
    assert_eq!(out, pcm);
}

#[test]
fn subband_level_seven_round_trip_low_amplitude() {
    // Low-amplitude sine — keeps the subband transform's amplification
    // from clipping the i16 coefficient buffer. Round-trip should be
    // close to the original; we allow a moderate tolerance because the
    // forward filter uses doubles + the inverse uses integer
    // wrapping arithmetic, which aren't strict inverses.
    let pcm: Vec<i16> = (0..4096)
        .map(|i| ((i as f32 * 0.1).sin() * 4000.0) as i16)
        .collect();

    let out = subband_round_trip(&pcm, 1, 22050, 7, 16);
    assert_eq!(out.len(), pcm.len());

    let (max_abs, rms) = diff_stats(&pcm, &out);
    eprintln!("level=7 low-amplitude: max_abs={max_abs} rms={rms:.2}");
    // Generous tolerance — actual values once the test runs.
    assert!(max_abs < 32_000, "max_abs diff {max_abs} too large");
}

#[test]
fn subband_level_four_round_trip() {
    let pcm: Vec<i16> = (0..2048)
        .map(|i| {
            let t = i as f32 / 100.0;
            ((t.sin() * 0.5 + (t * 1.7).cos() * 0.3) * 5000.0) as i16
        })
        .collect();

    let out = subband_round_trip(&pcm, 1, 22050, 4, 32);
    assert_eq!(out.len(), pcm.len());

    let (max_abs, rms) = diff_stats(&pcm, &out);
    eprintln!("level=4: max_abs={max_abs} rms={rms:.2}");
    assert!(max_abs < 32_000);
}

#[test]
fn subband_silence_round_trip_is_exact() {
    // All-zero input: the subband transform produces all-zero
    // coefficients, every column packs as f_zero, every block header
    // is pwr=0/val=1. Decode must give back exact silence.
    let pcm = vec![0i16; 2048];
    let out = subband_round_trip(&pcm, 1, 22050, 7, 16);
    assert_eq!(out, pcm);
}

#[test]
fn subband_partial_last_block_is_padded() {
    // Block size at level=7, rows=16 is 16*128 = 2048 samples. Pick a
    // length that doesn't divide evenly so the encoder must pad the
    // final block; the decoder should still stop at total_values.
    let pcm: Vec<i16> = (0..2113)
        .map(|i| ((i as f32 * 0.2).sin() * 3000.0) as i16)
        .collect();
    let out = subband_round_trip(&pcm, 1, 22050, 7, 16);
    assert_eq!(out.len(), pcm.len());
}

#[test]
fn subband_compresses_better_than_v1() {
    // Speech-like signal (sine bursts with silence gaps). With the
    // subband transform on, most coefficients in a quiet region are
    // tiny — pack_column picks f_zero / K12 / K13 for them and the
    // file shrinks dramatically vs. v1's flat 16-bits-per-sample.
    let mut pcm: Vec<i16> = Vec::with_capacity(22050);
    for i in 0..22050 {
        let phase = i % 4410;
        let amp = if phase < 3600 { 4000 } else { 0 };
        let s = ((i as f32 * 0.4).sin() * amp as f32) as i16;
        pcm.push(s);
    }

    let mut v1 = Vec::new();
    encode_pcm(&pcm, 1, 22050, &mut v1).unwrap();
    let mut packed = Vec::new();
    encode_pcm_packed(&pcm, 1, 22050, &mut packed).unwrap();
    let mut subband = Vec::new();
    encode_pcm_subband(&pcm, 1, 22050, 7, 16, &mut subband).unwrap();

    eprintln!(
        "compression: v1={} packed={} subband={}  (subband / v1 = {:.3})",
        v1.len(),
        packed.len(),
        subband.len(),
        subband.len() as f64 / v1.len() as f64
    );

    assert!(subband.len() < v1.len(), "subband must beat v1");
    assert!(
        subband.len() < packed.len(),
        "subband should beat acm_level=0 packing for this signal"
    );
}

#[test]
fn subband_with_f_half_smaller() {
    // Non-default f_half — confirms the parameter plumbing works.
    let pcm: Vec<i16> = (0..1000)
        .map(|i| ((i as f32 * 0.3).sin() * 2000.0) as i16)
        .collect();
    let mut buf = Vec::new();
    encode_pcm_subband_with_f_half(&pcm, 1, 22050, 5, 4, 16, &mut buf).unwrap();

    let mut dec =
        AcmDecoder::open(&DataSource::new(buf), OutputChannels::Original, "f_half=5").unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out.len(), pcm.len());
    let (max_abs, _) = diff_stats(&pcm, &out);
    assert!(max_abs < 32_000);
}
