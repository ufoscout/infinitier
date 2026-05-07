#![doc = include_str!("../readme.md")]

mod bitwriter;
mod packer;
mod subband;

use bitwriter::BitWriter;

use std::io::{self, Read, Write};

use hound::{SampleFormat, WavReader};
use thiserror::Error as ThisError;

/// 24-bit ACM signature (`0x97 28 03 01` little-endian → `0x00032897`
/// when read as a 24-bit value).
const ACM_ID: u32 = 0x032897;

/// Default block size (`acm_rows`). 512 is a comfortable middle ground:
/// big enough that header overhead per block is negligible, small enough
/// to keep latency low. 12-bit field, must be > 0 and < 4096.
pub const DEFAULT_ACM_ROWS: u32 = 512;

/// Quantizer power used by the v1 encoder. `count = 1 << pwr` entries on
/// each side of zero. With `val = 1` and `pwr = 15`, the amplitude
/// lookup table the decoder builds is exactly the identity over the
/// full i16 range, so we get lossless quantization.
const ENC_PWR: u32 = 15;

/// Quantizer step size. Combined with `pwr = 15`, `val = 1` produces the
/// identity amplitude table.
const ENC_VAL: u32 = 1;

/// Filler book index used per column. `f_linear` accepts `ind` in 3..=16
/// and reads `ind` bits per value; we use `ind = 16` so each value
/// covers the full i16 range exactly.
const ENC_IND: u32 = 16;

// ─── WAVC container constants ────────────────────────────────────────────────

/// Total length of the WAVC header in bytes — also written into the
/// "pointer to ACM data" field at offset 0x10 of the same header.
const WAVC_HEADER_SIZE: u32 = 28;

/// Bits per sample written into the WAVC header. The decoder always
/// emits `i16` PCM, so this is always 16.
const WAVC_BITS_PER_SAMPLE: u16 = 16;

/// WAVC files must use this sample rate per the engine spec.
const WAVC_REQUIRED_SAMPLE_RATE: u32 = 22050;

/// Magic value that goes in the "unused" word at offset 0x1a of the
/// WAVC header. Match what the games' WAVC files have so a reader
/// that strict-checks this field still accepts our output.
const WAVC_UNUSED_MAGIC: u16 = 0x777e;

// ─── Error type ───────────────────────────────────────────────────────────────

pub type Result<T> = std::result::Result<T, AcmEncodeError>;

#[derive(Debug, ThisError)]
pub enum AcmEncodeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("wav error: {0}")]
    Wav(#[from] hound::Error),
    #[error(
        "unsupported PCM format: bits_per_sample={bits}, sample_format={fmt:?} (only 16-bit \
         integer PCM is supported)"
    )]
    UnsupportedPcmFormat { bits: u16, fmt: SampleFormat },
    #[error("invalid channel count: {0} (must be 1 or 2)")]
    InvalidChannels(u32),
    #[error("sample rate too low: {0} Hz (must be >= 4096)")]
    SampleRateTooLow(u32),
    #[error("invalid block size {0}: must be in 1..4096 (and even when channels = 2)")]
    InvalidBlockSize(u32),
    #[error("invalid acm_level {0}: must be in 0..=15")]
    InvalidAcmLevel(u32),
    #[error("invalid f_half {0}: must be ≥ 1")]
    InvalidFHalf(u32),
    #[error("WAVC requires sample_rate = 22050 Hz (got {0})")]
    WavcInvalidSampleRate(u32),
    #[error("input is empty")]
    EmptyInput,
    #[error("too many samples: {0} (max 4294967295)")]
    TooManySamples(usize),
}

impl From<AcmEncodeError> for io::Error {
    fn from(err: AcmEncodeError) -> Self {
        match err {
            AcmEncodeError::Io(e) => e,
            other => io::Error::other(other),
        }
    }
}

// ─── Bit writer ───────────────────────────────────────────────────────────────

// ─── Encoder ─────────────────────────────────────────────────────────────────

/// Encode interleaved 16-bit PCM samples into an ACM bitstream and write
/// it to `out`.
///
/// `channels` must be 1 or 2; `sample_rate` must be ≥ 4096 (both checked
/// by the decoder's header parser). Stereo samples are interleaved
/// frame-by-frame, the same convention the decoder produces.
pub fn encode_pcm<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    out: &mut W,
) -> Result<()> {
    encode_pcm_with_block_size(samples, channels, sample_rate, DEFAULT_ACM_ROWS, out)
}

/// Like [`encode_pcm`] but lets the caller pick `acm_rows` (the block
/// size, samples per block / channel-frames per block when stereo).
pub fn encode_pcm_with_block_size<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    acm_rows: u32,
    out: &mut W,
) -> Result<()> {
    if !(1..=2).contains(&channels) {
        return Err(AcmEncodeError::InvalidChannels(channels));
    }
    if sample_rate < 4096 {
        return Err(AcmEncodeError::SampleRateTooLow(sample_rate));
    }
    if acm_rows == 0 || acm_rows >= 4096 {
        return Err(AcmEncodeError::InvalidBlockSize(acm_rows));
    }
    if channels == 2 && !acm_rows.is_multiple_of(2) {
        // The decoder will silently drop a stereo frame that straddles a
        // block boundary, which would corrupt the trail of the file.
        return Err(AcmEncodeError::InvalidBlockSize(acm_rows));
    }
    if samples.is_empty() {
        return Err(AcmEncodeError::EmptyInput);
    }
    if samples.len() > u32::MAX as usize {
        return Err(AcmEncodeError::TooManySamples(samples.len()));
    }

    let total_values = samples.len() as u32;
    let mut bw = BitWriter::new(out);

    // ── Header ────────────────────────────────────────────────────────────
    // 24-bit ACM ID + 8-bit version + low/high 16 bits of total_values
    // + 16-bit channels + 16-bit rate + 4-bit acm_level + 12-bit acm_rows.
    bw.put_bits(ACM_ID, 24)?;
    bw.put_bits(1, 8)?;
    bw.put_bits(total_values & 0xFFFF, 16)?;
    bw.put_bits(total_values >> 16, 16)?;
    bw.put_bits(channels, 16)?;
    bw.put_bits(sample_rate, 16)?;
    bw.put_bits(0, 4)?; // acm_level
    bw.put_bits(acm_rows, 12)?;

    // ── Blocks ────────────────────────────────────────────────────────────
    // With acm_level = 0, acm_cols = 1, so each block stores `acm_rows`
    // samples in a single column.
    let block_len = acm_rows as usize;
    let n_blocks = samples.len().div_ceil(block_len);

    for b in 0..n_blocks {
        // Block header: pwr=15, val=1.
        bw.put_bits(ENC_PWR, 4)?;
        bw.put_bits(ENC_VAL, 16)?;

        // Single column: ind=16, then 16 bits per sample as `b = sample
        // + 32768` (the decoder's `b - middle` recovers the signed
        // value, and the val=1 amplitude table is the identity).
        bw.put_bits(ENC_IND, 5)?;

        let block_start = b * block_len;
        for r in 0..block_len {
            let i = block_start + r;
            let s = if i < samples.len() {
                samples[i]
            } else {
                0i16 // pad the trailing block; decoder stops at total_values.
            };
            let b_val = (s as i32 + 32768) as u32;
            bw.put_bits(b_val & 0xFFFF, 16)?;
        }
    }

    bw.finish()?;
    Ok(())
}

/// Encode the contents of a 16-bit signed-integer PCM RIFF/WAVE stream
/// into an ACM bitstream. Lossless.
pub fn encode_wav<R: Read, W: Write>(reader: R, writer: &mut W) -> Result<()> {
    let mut wav = WavReader::new(reader)?;
    let spec = wav.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(AcmEncodeError::UnsupportedPcmFormat {
            bits: spec.bits_per_sample,
            fmt: spec.sample_format,
        });
    }
    let samples: Vec<i16> = wav
        .samples::<i16>()
        .collect::<std::result::Result<_, _>>()?;
    encode_pcm(&samples, spec.channels as u32, spec.sample_rate, writer)
}

/// Encode interleaved 16-bit PCM into an ACM bitstream using the
/// ported DLTCEP packer (`acm_level = 0` — the subband transform stays
/// off until the encoder pipeline drives it end-to-end).
///
/// At `acm_level = 0` the packer's GCD-based quantizer is exact (no
/// rounding loss) and the per-column book selection is lossless, so the
/// output round-trips through `infinitier_acm_decoder` bit-for-bit
/// while typically being smaller than [`encode_pcm`]'s output for
/// signals with structure (silence, repeated values, low-amplitude
/// passages).
pub fn encode_pcm_packed<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    out: &mut W,
) -> Result<()> {
    encode_pcm_packed_with_block_size(samples, channels, sample_rate, DEFAULT_ACM_ROWS, out)
}

/// As [`encode_pcm_packed`] with a configurable block size (`acm_rows`).
pub fn encode_pcm_packed_with_block_size<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    acm_rows: u32,
    out: &mut W,
) -> Result<()> {
    if !(1..=2).contains(&channels) {
        return Err(AcmEncodeError::InvalidChannels(channels));
    }
    if sample_rate < 4096 {
        return Err(AcmEncodeError::SampleRateTooLow(sample_rate));
    }
    if acm_rows == 0 || acm_rows >= 4096 {
        return Err(AcmEncodeError::InvalidBlockSize(acm_rows));
    }
    if channels == 2 && !acm_rows.is_multiple_of(2) {
        return Err(AcmEncodeError::InvalidBlockSize(acm_rows));
    }
    if samples.is_empty() {
        return Err(AcmEncodeError::EmptyInput);
    }
    if samples.len() > u32::MAX as usize {
        return Err(AcmEncodeError::TooManySamples(samples.len()));
    }

    let total_values = samples.len() as u32;
    let mut bw = BitWriter::new(out);

    // Header — same layout as the v1 encoder, acm_level = 0.
    bw.put_bits(ACM_ID, 24)?;
    bw.put_bits(1, 8)?;
    bw.put_bits(total_values & 0xFFFF, 16)?;
    bw.put_bits(total_values >> 16, 16)?;
    bw.put_bits(channels, 16)?;
    bw.put_bits(sample_rate, 16)?;
    bw.put_bits(0, 4)?; // acm_level
    bw.put_bits(acm_rows, 12)?;

    let block_len = acm_rows as usize;
    let n_blocks = samples.len().div_ceil(block_len);

    let mut packer = packer::ValuePacker::new(block_len, 1, None);
    let mut buf = vec![0i16; block_len];

    for b in 0..n_blocks {
        let block_start = b * block_len;
        for (r, slot) in buf.iter_mut().enumerate() {
            let i = block_start + r;
            *slot = if i < samples.len() {
                samples[i]
            } else {
                0i16
            };
        }
        packer.add_block(&buf, &mut bw)?;
    }

    bw.finish()?;
    Ok(())
}

/// Default subband filter half-length used by [`encode_pcm_subband`].
/// `snd2acm.cpp` uses 11; the resulting filter has `f_len = 21`.
pub const DEFAULT_F_HALF: u32 = 11;

/// Encode interleaved 16-bit PCM via the **full ACM pipeline** —
/// forward subband transform (`SubbandCoder`) + per-block quantization
/// and per-column filler-book packing (`ValuePacker`).
///
/// `acm_level` is the pyramid depth (the `levels` field of the C++
/// header — `1 << acm_level` columns per block, ≤ 15). `acm_rows` is
/// the row count per block. Together they define a block size of
/// `acm_rows × (1 << acm_level)` coefficients.
///
/// The forward subband transform uses double-precision floats, then
/// truncates to `i16`. Combined with the lossless GCD quantizer and the
/// integer inverse transform on the decoder side, the round-trip
/// preserves the signal up to small filter-rounding noise — typically
/// a few percent of the i16 range.
pub fn encode_pcm_subband<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    acm_level: u32,
    acm_rows: u32,
    out: &mut W,
) -> Result<()> {
    encode_pcm_subband_with_f_half(
        samples,
        channels,
        sample_rate,
        DEFAULT_F_HALF,
        acm_level,
        acm_rows,
        out,
    )
}

/// As [`encode_pcm_subband`] but lets the caller pick the subband
/// filter's half-length. `f_half = 11` matches the DLTCEP encoder's
/// default and gives a 21-tap filter.
pub fn encode_pcm_subband_with_f_half<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    f_half: u32,
    acm_level: u32,
    acm_rows: u32,
    out: &mut W,
) -> Result<()> {
    if !(1..=2).contains(&channels) {
        return Err(AcmEncodeError::InvalidChannels(channels));
    }
    if sample_rate < 4096 {
        return Err(AcmEncodeError::SampleRateTooLow(sample_rate));
    }
    if acm_level >= 16 {
        return Err(AcmEncodeError::InvalidAcmLevel(acm_level));
    }
    if f_half == 0 {
        return Err(AcmEncodeError::InvalidFHalf(f_half));
    }
    if acm_rows == 0 || acm_rows >= 4096 {
        return Err(AcmEncodeError::InvalidBlockSize(acm_rows));
    }
    if samples.is_empty() {
        return Err(AcmEncodeError::EmptyInput);
    }
    if samples.len() > u32::MAX as usize {
        return Err(AcmEncodeError::TooManySamples(samples.len()));
    }

    let sb_size = 1usize << acm_level;
    let block_size = acm_rows as usize * sb_size;

    if channels == 2 && !block_size.is_multiple_of(2) {
        // The decoder will silently drop a stereo frame that straddles
        // a block boundary — refuse the geometry up-front.
        return Err(AcmEncodeError::InvalidBlockSize(acm_rows));
    }

    let total_values = samples.len() as u32;
    let mut bw = BitWriter::new(out);

    // ── ACM file header (matches `struct ACM_Header` from general.h) ─────
    bw.put_bits(ACM_ID, 24)?;
    bw.put_bits(1, 8)?;
    bw.put_bits(total_values & 0xFFFF, 16)?;
    bw.put_bits(total_values >> 16, 16)?;
    bw.put_bits(channels, 16)?;
    bw.put_bits(sample_rate, 16)?;
    bw.put_bits(acm_level, 4)?;
    bw.put_bits(acm_rows, 12)?;

    let mut coder = subband::SubbandCoder::new(f_half as usize, acm_level as usize);
    let mut packer = packer::ValuePacker::new(acm_rows as usize, sb_size, None);

    // Pad with `init_size` zeros so the filter's pyramid latency is
    // fully consumed and the returned coefficient count equals the
    // input sample count exactly. Mirrors snd2acm.cpp's
    // `left_to_filter = samples + coder->get_init_size()`.
    let init = coder.init_size();
    let mut input = Vec::with_capacity(samples.len() + init);
    input.extend_from_slice(samples);
    input.resize(samples.len() + init, 0);

    let mut coeffs = Vec::<i64>::new();
    let n_coeffs = coder.filter_data(&input, &mut coeffs);

    // Stream coefficients into row-major blocks; pack each full block,
    // pad the trailing partial block with zeros.
    let mut buf = vec![0i16; block_size];
    let mut bp = 0usize;
    for &coeff in coeffs.iter().take(n_coeffs) {
        // Clamp to i16 — the C++ does the same and counts clipping
        // events as warnings. Real-world signals only saturate when
        // the lifting transform amplifies a transient beyond ±32768.
        let c = coeff.clamp(i16::MIN as i64, i16::MAX as i64) as i16;
        buf[bp] = c;
        bp += 1;
        if bp == block_size {
            packer.add_block(&buf, &mut bw)?;
            bp = 0;
        }
    }
    if bp > 0 {
        for v in &mut buf[bp..] {
            *v = 0;
        }
        packer.add_block(&buf, &mut bw)?;
    }

    bw.finish()?;
    Ok(())
}

/// Encode a 16-bit signed-integer PCM RIFF/WAVE stream into an ACM
/// bitstream with the full subband + packer pipeline. Picks reasonable
/// defaults — `f_half = 11`, `acm_level = 7`, `acm_rows = 16` —
/// matching `snd2acm.cpp`'s defaults.
pub fn encode_wav_subband<R: Read, W: Write>(reader: R, writer: &mut W) -> Result<()> {
    let mut wav = WavReader::new(reader)?;
    let spec = wav.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(AcmEncodeError::UnsupportedPcmFormat {
            bits: spec.bits_per_sample,
            fmt: spec.sample_format,
        });
    }
    let samples: Vec<i16> = wav
        .samples::<i16>()
        .collect::<std::result::Result<_, _>>()?;
    encode_pcm_subband(
        &samples,
        spec.channels as u32,
        spec.sample_rate,
        7,  // acm_level — DLTCEP default
        16, // acm_rows — DLTCEP default
        writer,
    )
}

// ─── WAVC output ──────────────────────────────────────────────────────────────

/// WAVC is the engine's container for ACM: a 28-byte header followed by
/// a regular ACM bitstream. The header must declare the PCM size, so we
/// can't stream-write it — encode the ACM body into an in-memory buffer
/// first, fill in `compressed_size`, then emit the header + body.
fn write_wavc<W: Write>(
    out: &mut W,
    samples_len: usize,
    channels: u32,
    sample_rate: u32,
    acm_body: &[u8],
) -> Result<()> {
    require_wavc_metadata(channels, sample_rate)?;

    // Per the spec each PCM sample is 16-bit, so the uncompressed byte
    // size is `total_values * 2`. Use a checked multiply — a u32
    // sample count near the limit overflows otherwise.
    let uncompressed = (samples_len as u64).saturating_mul(2);
    let uncompressed = u32::try_from(uncompressed)
        .map_err(|_| AcmEncodeError::TooManySamples(samples_len))?;
    let compressed = u32::try_from(acm_body.len())
        .map_err(|_| AcmEncodeError::TooManySamples(acm_body.len()))?;

    let mut header = [0u8; WAVC_HEADER_SIZE as usize];
    header[0..4].copy_from_slice(b"WAVC");
    header[4..8].copy_from_slice(b"V1.0");
    header[8..12].copy_from_slice(&uncompressed.to_le_bytes());
    header[12..16].copy_from_slice(&compressed.to_le_bytes());
    header[16..20].copy_from_slice(&WAVC_HEADER_SIZE.to_le_bytes());
    header[20..22].copy_from_slice(&(channels as u16).to_le_bytes());
    header[22..24].copy_from_slice(&WAVC_BITS_PER_SAMPLE.to_le_bytes());
    header[24..26].copy_from_slice(&(sample_rate as u16).to_le_bytes());
    header[26..28].copy_from_slice(&WAVC_UNUSED_MAGIC.to_le_bytes());

    out.write_all(&header)?;
    out.write_all(acm_body)?;
    Ok(())
}

fn require_wavc_metadata(channels: u32, sample_rate: u32) -> Result<()> {
    if !(1..=2).contains(&channels) {
        return Err(AcmEncodeError::InvalidChannels(channels));
    }
    if sample_rate != WAVC_REQUIRED_SAMPLE_RATE {
        return Err(AcmEncodeError::WavcInvalidSampleRate(sample_rate));
    }
    Ok(())
}

/// Encode interleaved 16-bit PCM as a WAVC file using the lossless v1
/// path internally. `sample_rate` must equal 22050 Hz — the WAVC
/// container is hard-pinned to that rate per the engine spec.
pub fn encode_pcm_wavc<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    out: &mut W,
) -> Result<()> {
    require_wavc_metadata(channels, sample_rate)?;
    let mut acm = Vec::new();
    encode_pcm(samples, channels, sample_rate, &mut acm)?;
    write_wavc(out, samples.len(), channels, sample_rate, &acm)
}

/// Encode interleaved 16-bit PCM as a WAVC file using the per-column
/// packer (lossless, typically 0.7–0.9× the size of [`encode_pcm_wavc`]).
/// `sample_rate` must equal 22050 Hz.
pub fn encode_pcm_packed_wavc<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    out: &mut W,
) -> Result<()> {
    require_wavc_metadata(channels, sample_rate)?;
    let mut acm = Vec::new();
    encode_pcm_packed(samples, channels, sample_rate, &mut acm)?;
    write_wavc(out, samples.len(), channels, sample_rate, &acm)
}

/// Encode interleaved 16-bit PCM as a WAVC file using the full
/// subband+packer pipeline (typically 0.3–0.5× compression but
/// slightly lossy). `sample_rate` must equal 22050 Hz.
pub fn encode_pcm_subband_wavc<W: Write>(
    samples: &[i16],
    channels: u32,
    sample_rate: u32,
    acm_level: u32,
    acm_rows: u32,
    out: &mut W,
) -> Result<()> {
    require_wavc_metadata(channels, sample_rate)?;
    let mut acm = Vec::new();
    encode_pcm_subband(samples, channels, sample_rate, acm_level, acm_rows, &mut acm)?;
    write_wavc(out, samples.len(), channels, sample_rate, &acm)
}

/// Encode the contents of a 16-bit signed-integer PCM RIFF/WAVE stream
/// as a WAVC file using the lossless v1 path. Fails with
/// [`AcmEncodeError::WavcInvalidSampleRate`] if the input WAV is not at
/// 22050 Hz.
pub fn encode_wav_wavc<R: Read, W: Write>(reader: R, writer: &mut W) -> Result<()> {
    let mut wav = WavReader::new(reader)?;
    let spec = wav.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(AcmEncodeError::UnsupportedPcmFormat {
            bits: spec.bits_per_sample,
            fmt: spec.sample_format,
        });
    }
    let samples: Vec<i16> = wav
        .samples::<i16>()
        .collect::<std::result::Result<_, _>>()?;
    encode_pcm_wavc(&samples, spec.channels as u32, spec.sample_rate, writer)
}

/// Encode the contents of a 16-bit signed-integer PCM RIFF/WAVE stream
/// as a WAVC file using the full subband + packer pipeline. The
/// resulting file is what the engine actually plays — typical real
/// game-data compression ratios. Fails with
/// [`AcmEncodeError::WavcInvalidSampleRate`] if the input WAV is not at
/// 22050 Hz.
pub fn encode_wav_subband_wavc<R: Read, W: Write>(reader: R, writer: &mut W) -> Result<()> {
    let mut wav = WavReader::new(reader)?;
    let spec = wav.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(AcmEncodeError::UnsupportedPcmFormat {
            bits: spec.bits_per_sample,
            fmt: spec.sample_format,
        });
    }
    let samples: Vec<i16> = wav
        .samples::<i16>()
        .collect::<std::result::Result<_, _>>()?;
    encode_pcm_subband_wavc(
        &samples,
        spec.channels as u32,
        spec.sample_rate,
        7,  // acm_level — DLTCEP default
        16, // acm_rows — DLTCEP default
        writer,
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::DataSource;

use super::*;

    #[test]
    fn rejects_invalid_channels() {
        let mut out = Vec::new();
        let err = encode_pcm(&[0, 1, 2, 3], 3, 22050, &mut out).unwrap_err();
        assert!(matches!(err, AcmEncodeError::InvalidChannels(3)));
    }

    #[test]
    fn rejects_low_sample_rate() {
        let mut out = Vec::new();
        let err = encode_pcm(&[0, 1, 2, 3], 1, 100, &mut out).unwrap_err();
        assert!(matches!(err, AcmEncodeError::SampleRateTooLow(100)));
    }

    #[test]
    fn rejects_odd_block_for_stereo() {
        let mut out = Vec::new();
        let err = encode_pcm_with_block_size(&[0, 1, 2, 3], 2, 22050, 7, &mut out).unwrap_err();
        assert!(matches!(err, AcmEncodeError::InvalidBlockSize(7)));
    }

    #[test]
    fn rejects_empty_input() {
        let mut out = Vec::new();
        let err = encode_pcm(&[], 1, 22050, &mut out).unwrap_err();
        assert!(matches!(err, AcmEncodeError::EmptyInput));
    }


fn round_trip(samples: &[i16], channels: u32, sample_rate: u32) -> Vec<i16> {
    let mut buf = Vec::new();
    encode_pcm(samples, channels, sample_rate, &mut buf).expect("encode failed");
    let dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "round_trip",
    )
    .expect("open failed");
    assert_eq!(dec.info.channels, channels, "channel count must round-trip");
    assert_eq!(
        dec.info.rate, sample_rate,
        "sample rate must round-trip"
    );
    assert_eq!(
        dec.info.total_values as usize,
        samples.len(),
        "total_values must round-trip"
    );
    let mut dec = dec;
    dec.decode_all().expect("decode failed")
}

#[test]
fn round_trip_mono_short() {
    let pcm: Vec<i16> = vec![0, 1, -1, 12345, -12345, 32767, -32768, 100, 200, 300];
    let out = round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_mono_one_block_exact() {
    // Exactly one default-sized block (512 samples).
    let pcm: Vec<i16> = (0..512)
        .map(|i| (i as i16).wrapping_mul(37))
        .collect();
    let out = round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_mono_partial_last_block() {
    // 512 + 17 samples — last block is partial; the decoder must stop
    // at total_values, ignoring the encoder's zero padding.
    let pcm: Vec<i16> = (0..529)
        .map(|i| ((i * 91) as i16).wrapping_sub((i * 7) as i16))
        .collect();
    let out = round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_stereo() {
    // Stereo, multiple full blocks.
    let pcm: Vec<i16> = (0..2048)
        .map(|i| {
            if i % 2 == 0 {
                ((i / 2) as i16).wrapping_mul(13)
            } else {
                -(((i / 2) as i16).wrapping_mul(13))
            }
        })
        .collect();
    let out = round_trip(&pcm, 2, 44100);
    assert_eq!(out, pcm);
}

#[test]
fn round_trip_extreme_values() {
    // Edge values: i16::MIN, i16::MAX, 0, ±1 — exercises the b±middle
    // boundaries.
    let pcm = vec![
        i16::MIN,
        i16::MIN + 1,
        -1,
        0,
        1,
        i16::MAX - 1,
        i16::MAX,
        // Repeat to span >1 block.
        i16::MIN,
        i16::MAX,
        0,
    ];
    let mut padded = Vec::new();
    for _ in 0..200 {
        padded.extend_from_slice(&pcm);
    }
    let out = round_trip(&padded, 1, 22050);
    assert_eq!(out, padded);
}

#[test]
fn round_trip_small_block_size() {
    let pcm: Vec<i16> = (0..1000)
        .map(|i| (i as i16).wrapping_mul(11))
        .collect();
    let mut buf = Vec::new();
    encode_pcm_with_block_size(&pcm, 1, 22050, 8, &mut buf).unwrap();
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "small_block",
    )
    .unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out, pcm);
}

/// Encode via the packer, decode via AcmDecoder, return the decoded
/// samples for assertion in tests.
fn packer_round_trip(samples: &[i16], channels: u32, sample_rate: u32) -> Vec<i16> {
    let mut buf = Vec::new();
    encode_pcm_packed(samples, channels, sample_rate, &mut buf).expect("encode failed");
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "packer_round_trip",
    )
    .expect("open failed");
    assert_eq!(
        dec.info.channels, channels,
        "channel count must round-trip"
    );
    assert_eq!(dec.info.rate, sample_rate, "sample rate must round-trip");
    assert_eq!(
        dec.info.total_values as usize,
        samples.len(),
        "total_values must round-trip"
    );
    dec.decode_all().expect("decode failed")
}

#[test]
fn packer_round_trip_silence() {
    // All-zero input — every column should pack as f_zero (ind=0).
    let pcm = vec![0i16; 256];
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_small_amplitude_picks_huffman_books() {
    // Values in {-1, 0, 1} — pack_column should pick K12/K13/T15.
    let pcm: Vec<i16> = (0..512).map(|i| ((i % 3) as i16) - 1).collect();
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_full_i16_range() {
    // Wide-amplitude signal — falls into the default linear branch.
    let pcm: Vec<i16> = (0..1024)
        .map(|i| ((i as f32 * 0.1).sin() * 32000.0) as i16)
        .collect();
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_extreme_values() {
    // Edge values exercise the b±middle boundaries of make_linear at
    // bits=16, plus stress the granulator's pwr derivation.
    let mut pcm = Vec::new();
    for _ in 0..200 {
        pcm.extend_from_slice(&[i16::MIN, i16::MIN + 1, -1, 0, 1, i16::MAX - 1, i16::MAX]);
    }
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_partial_last_block() {
    // Block size doesn't divide len → trailing partial block padded
    // with zeros; the decoder must stop at the encoded total_values.
    let pcm: Vec<i16> = (0..529)
        .map(|i| ((i * 91) as i16).wrapping_sub((i * 7) as i16))
        .collect();
    let out = packer_round_trip(&pcm, 1, 22050);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_stereo() {
    let pcm: Vec<i16> = (0..2048)
        .map(|i| {
            if i % 2 == 0 {
                ((i / 2) as i16).wrapping_mul(13)
            } else {
                -(((i / 2) as i16).wrapping_mul(13))
            }
        })
        .collect();
    let out = packer_round_trip(&pcm, 2, 44100);
    assert_eq!(out, pcm);
}

#[test]
fn packer_round_trip_small_block_size() {
    let pcm: Vec<i16> = (0..1000)
        .map(|i| (i as i16).wrapping_mul(11))
        .collect();
    let mut buf = Vec::new();
    encode_pcm_packed_with_block_size(&pcm, 1, 22050, 8, &mut buf).unwrap();
    let mut dec = AcmDecoder::open(
        &DataSource::new(buf),
        OutputChannels::Original,
        "packer_small_block",
    )
    .unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out, pcm);
}

#[test]
fn packer_typically_compresses_below_v1() {
    // For a signal with structure (silence + occasional pulses), the
    // packer's f_zero/Huffman books should produce a noticeably
    // smaller bitstream than v1's flat 16-bits-per-sample encoding.
    let mut pcm = vec![0i16; 4096];
    // Sparse pulses — long runs of zeros surround a few non-zero
    // samples.
    for (i, x) in pcm.iter_mut().enumerate() {
        if i % 64 == 0 {
            *x = 1;
        }
    }
    let mut v1 = Vec::new();
    encode_pcm(&pcm, 1, 22050, &mut v1).unwrap();
    let mut packed = Vec::new();
    encode_pcm_packed(&pcm, 1, 22050, &mut packed).unwrap();
    assert!(
        packed.len() < v1.len(),
        "packer should compress sparse signal: v1={} packed={}",
        v1.len(),
        packed.len()
    );
}

#[test]
fn round_trip_wav_input() {
    // Build a small in-memory RIFF WAV via hound, run it through
    // encode_wav, then decode through AcmDecoder and compare.
    let pcm: Vec<i16> = (0..4096)
        .map(|i| ((i as f32 * 0.05).sin() * 16000.0) as i16)
        .collect();
    let mut wav = Cursor::new(Vec::<u8>::new());
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 22050,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::new(&mut wav, spec).unwrap();
        for &s in &pcm {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
    }
    let wav_bytes = wav.into_inner();

    let mut acm = Vec::new();
    encode_wav(Cursor::new(wav_bytes), &mut acm).unwrap();

    let mut dec = AcmDecoder::open(
        &DataSource::new(acm),
        OutputChannels::Original,
        "wav_round_trip",
    )
    .unwrap();
    let out = dec.decode_all().unwrap();
    assert_eq!(out, pcm);
}

}
