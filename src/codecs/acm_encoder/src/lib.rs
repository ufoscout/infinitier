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
    if channels == 2 && acm_rows % 2 != 0 {
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
    if channels == 2 && acm_rows % 2 != 0 {
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
        for r in 0..block_len {
            let i = block_start + r;
            buf[r] = if i < samples.len() {
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

#[cfg(test)]
mod tests {
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

}
