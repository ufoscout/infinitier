#![doc = include_str!("../readme.md")]

use std::io;
use std::path::Path;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use log::debug;
use thiserror::Error as ThisError;

use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::{DataSource, DataTrait, Reader};

const RIFF_MAGIC: &[u8; 4] = b"RIFF";
const WAVC_MAGIC: &[u8; 4] = b"WAVC";

pub type Result<T> = std::result::Result<T, WavError>;

#[derive(Debug, ThisError)]
pub enum WavError {
    #[error("not a WAV or WAVC file: unknown magic {0:?}")]
    UnknownFormat([u8; 4]),
    #[error(
        "unsupported PCM format: bits_per_sample={bits}, sample_format={fmt:?} (only 16-bit \
         integer PCM is supported)"
    )]
    UnsupportedPcmFormat { bits: u16, fmt: SampleFormat },
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("hound error: {0}")]
    Wav(#[from] hound::Error),
    #[error("acm decoder error: {0}")]
    Acm(#[from] infinitier_acm_decoder::AcmError),
}

impl From<WavError> for io::Error {
    fn from(err: WavError) -> Self {
        match err {
            WavError::Io(e) => e,
            other => io::Error::other(other),
        }
    }
}

/// Stream metadata, mirroring [`infinitier_acm_decoder::AcmInfo`] so callers
/// can treat both decoders uniformly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavInfo {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    /// Total interleaved PCM values the stream will produce
    /// (`channels × frames`).
    pub total_values: u32,
}

impl WavInfo {
    /// Number of frames (samples per channel) in the stream.
    pub fn frames(&self) -> u32 {
        self.total_values / self.channels.max(1) as u32
    }
}

/// The two `*.WAV` flavours used by Infinity Engine games.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavFormat {
    /// Standard `RIFF`/`WAVE` PCM.
    Wav,
    /// Interplay's WAVC: a 28-byte header wrapping an ACM stream.
    Wavc,
}

/// Streaming decoder for the two `*.WAV` flavours used by Infinity Engine
/// games. Pulls samples block-by-block from the underlying source so memory
/// stays bounded regardless of file size, mirroring
/// [`AcmDecoder`]'s API.
pub struct WavDecoder {
    info: WavInfo,
    datasource: DataSource,
    inner: WavInner,
}

enum WavInner {
    /// hound-driven RIFF/WAVE reader that owns its underlying file handle
    /// (or in-memory cursor) so the decoder is `'static`.
    Wav {
        reader: WavReader<Reader<Box<dyn DataTrait>>>,
        /// Total samples already produced; used to clamp output to the
        /// declared `total_values` if hound reports more.
        produced: u32,
    },
    Wavc(AcmDecoder),
}

impl std::fmt::Debug for WavDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WavDecoder")
            .field("format", &self.format())
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl WavDecoder {
    /// Open a WAV / WAVC stream from a [`DataSource`]. The first four bytes
    /// determine the flavour:
    ///
    /// - `RIFF` → standard PCM, decoded via [`hound`];
    /// - `WAVC` → Interplay's ACM-wrapping header, delegated to
    ///   [`AcmDecoder`] (which skips the 28-byte WAVC header itself).
    pub fn open(datasource: &DataSource) -> Result<Self> {
        let magic = peek_magic(datasource)?;
        match &magic {
            RIFF_MAGIC => Self::open_riff(datasource),
            WAVC_MAGIC => Self::open_wavc(datasource),
            _ => Err(WavError::UnknownFormat(magic)),
        }
    }

    fn open_riff(datasource: &DataSource) -> Result<Self> {
        let reader_box = datasource.reader()?;
        let reader = WavReader::new(reader_box)?;
        let spec = reader.spec();

        if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
            return Err(WavError::UnsupportedPcmFormat {
                bits: spec.bits_per_sample,
                fmt: spec.sample_format,
            });
        }

        // hound exposes `len()` as total interleaved samples, which is exactly
        // what we want for `total_values`.
        let total_values = reader.len();

        let info = WavInfo {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: spec.bits_per_sample,
            total_values,
        };

        debug!(
            "Opened WAV: channels={}, rate={}, bits={}, total_values={}",
            info.channels, info.sample_rate, info.bits_per_sample, info.total_values,
        );

        Ok(Self {
            info,
            datasource: datasource.clone(),
            inner: WavInner::Wav {
                reader,
                produced: 0,
            },
        })
    }

    fn open_wavc(datasource: &DataSource) -> Result<Self> {
        // AcmDecoder already validates the WAVC header (`'WAVC'`, `'V1.0'`,
        // 28-byte length) before reading the ACM body, so we don't re-parse
        // it here. 
        let decoder = AcmDecoder::open(datasource, OutputChannels::Original)?;
        let acm_info = decoder.info.clone();

        let info = WavInfo {
            channels: acm_info.channels as u16,
            sample_rate: acm_info.rate,
            bits_per_sample: acm_info.bits_per_sample(),
            total_values: acm_info.total_values,
        };

        debug!(
            "Opened WAVC: channels={}, rate={}, bits={}, total_values={}",
            info.channels, info.sample_rate, info.bits_per_sample, info.total_values,
        );

        Ok(Self {
            info,
            datasource: datasource.clone(),
            inner: WavInner::Wavc(decoder),
        })
    }

    /// Stream metadata.
    pub fn info(&self) -> &WavInfo {
        &self.info
    }

    /// Container flavour the decoder is reading.
    pub fn format(&self) -> WavFormat {
        match &self.inner {
            WavInner::Wav { .. } => WavFormat::Wav,
            WavInner::Wavc { .. } => WavFormat::Wavc,
        }
    }

    /// Decode the next chunk of PCM samples into `out`, returning the number
    /// of `i16` samples written. Returns `Ok(0)` only on natural end of
    /// stream. Samples are interleaved (frame-major) for stereo streams.
    pub fn read_samples(&mut self, out: &mut [i16]) -> Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        match &mut self.inner {
            WavInner::Wav { reader, produced } => {
                let remaining = self.info.total_values.saturating_sub(*produced) as usize;
                let want = out.len().min(remaining);
                if want == 0 {
                    return Ok(0);
                }

                let mut iter = reader.samples::<i16>();
                let mut written = 0usize;
                while written < want {
                    match iter.next() {
                        Some(Ok(s)) => {
                            out[written] = s;
                            written += 1;
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => break,
                    }
                }
                *produced += written as u32;
                Ok(written)
            }
            WavInner::Wavc(dec) => Ok(dec.read_samples(out)?),
        }
    }

    /// Decode the entire stream and return all PCM samples as interleaved
    /// signed 16-bit values.
    pub fn decode_all(&mut self) -> Result<Vec<i16>> {
        let total = self.info.total_values as usize;
        let mut samples = vec![0i16; total];
        let mut written = 0usize;
        while written < total {
            let n = self.read_samples(&mut samples[written..])?;
            if n == 0 {
                break;
            }
            written += n;
        }
        Ok(samples)
    }

    /// Decode the stream into a 16-bit PCM `RIFF`/`WAVE` file.
    pub fn decode_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let spec = WavSpec {
            channels: self.info.channels,
            sample_rate: self.info.sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec)?;
        let mut buf = [0i16; 4096];
        loop {
            let n = self.read_samples(&mut buf)?;
            if n == 0 {
                break;
            }
            for &s in &buf[..n] {
                writer.write_sample(s)?;
            }
        }
        writer.finalize()?;
        Ok(())
    }

    /// Rewind the decoder to the start of the stream by reopening the
    /// underlying [`DataSource`].
    pub fn reset(&mut self) -> Result<()> {
        let fresh = Self::open(&self.datasource)?;
        *self = fresh;
        Ok(())
    }
}

/// Read the first up-to-`n` bytes of a [`DataSource`] for header sniffing.
fn read_header(ds: &DataSource, n: usize) -> io::Result<Vec<u8>> {
    let mut reader = ds.reader()?;
    let mut buf = vec![0u8; n];
    let mut filled = 0;
    while filled < n {
        match reader.read(&mut buf[filled..])? {
            0 => break,
            got => filled += got,
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

/// Read the 4-byte magic at the start of a [`DataSource`].
fn peek_magic(ds: &DataSource) -> io::Result<[u8; 4]> {
    let header = read_header(ds, 4)?;
    if header.len() < 4 {
        return Ok([0; 4]);
    }
    Ok([header[0], header[1], header[2], header[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_magic_is_rejected() {
        let bytes = b"XYZW....".to_vec();
        let err = WavDecoder::open(&DataSource::new(bytes)).unwrap_err();
        assert!(matches!(err, WavError::UnknownFormat(_)));
    }

    #[test]
    fn truncated_input_is_rejected() {
        let bytes = b"WA".to_vec();
        let err = WavDecoder::open(&DataSource::new(bytes)).unwrap_err();
        // Two bytes pad to `[W, A, 0, 0]` and miss both magics.
        assert!(matches!(err, WavError::UnknownFormat(_)));
    }

}
