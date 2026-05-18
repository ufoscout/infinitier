//! Unified streaming audio decoder over the engine's two sound formats:
//! Interplay's ACM (`.ACM`) and the `.WAV` flavour shipped under that
//! extension (real RIFF/WAVE plus Interplay's WAVC).

use std::fmt;
use std::io;

use infinitier_acm_decoder::AcmDecoder;
use infinitier_wav_decoder::{WavDecoder, WavFormat};

/// Sound information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundInfo {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    /// Total interleaved PCM values the stream will produce
    /// (`channels × frames`).
    pub total_values: u32,
}

impl SoundInfo {
    /// Number of frames (samples per channel) in the stream.
    pub fn frames(&self) -> u32 {
        self.total_values / self.channels.max(1) as u32
    }
}

/// Sound format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFormat {
    /// Interplay ACM bitstream (`.ACM` resources).
    Acm,
    /// Standard `RIFF`/`WAVE` PCM.
    Wav,
    /// Interplay's WAVC: a 28-byte header wrapping an ACM stream.
    Wavc,
    /// Ogg-encapsulated Vorbis (Enhanced Editions ship these under
    /// the `.WAV` extension).
    Ogg,
}

impl fmt::Display for SoundFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoundFormat::Acm => f.write_str("ACM"),
            SoundFormat::Wav => f.write_str("WAV"),
            SoundFormat::Wavc => f.write_str("WAVC"),
            SoundFormat::Ogg => f.write_str("OGG"),
        }
    }
}

/// Streaming audio decoder for either of the two formats Infinity Engine
/// ships under `.ACM` / `.WAV` extensions.
#[derive(Debug)]
pub enum SoundDecoder {
    Acm(AcmDecoder),
    Wav(WavDecoder),
}

impl SoundDecoder {
    /// Resource name.
    pub fn name(&self) -> &str {
        match self {
            SoundDecoder::Acm(d) => d.name(),
            SoundDecoder::Wav(d) => d.name(),
        }
    }

    /// Stream format
    pub fn format(&self) -> SoundFormat {
        match self {
            SoundDecoder::Acm(_) => SoundFormat::Acm,
            SoundDecoder::Wav(d) => match d.format() {
                WavFormat::Wav => SoundFormat::Wav,
                WavFormat::Wavc => SoundFormat::Wavc,
                WavFormat::Ogg => SoundFormat::Ogg,
            },
        }
    }

    /// Stream metadata.
    pub fn info(&self) -> SoundInfo {
        match self {
            SoundDecoder::Acm(d) => SoundInfo {
                channels: d.info.channels as u16,
                sample_rate: d.info.rate,
                bits_per_sample: d.info.bits_per_sample(),
                total_values: d.info.total_values,
            },
            SoundDecoder::Wav(d) => {
                let i = d.info();
                SoundInfo {
                    channels: i.channels,
                    sample_rate: i.sample_rate,
                    bits_per_sample: i.bits_per_sample,
                    total_values: i.total_values,
                }
            }
        }
    }

    /// Decode the next chunk of PCM samples into `out`, returning the
    /// number of `i16` samples written. Returns `Ok(0)` only at the
    /// natural end of the stream.
    pub fn read_samples(&mut self, out: &mut [i16]) -> io::Result<usize> {
        match self {
            SoundDecoder::Acm(d) => d.read_samples(out).map_err(io::Error::from),
            SoundDecoder::Wav(d) => d.read_samples(out).map_err(io::Error::from),
        }
    }

    /// Decode the entire stream into a fresh `Vec<i16>` of interleaved PCM
    /// samples. Convenient for UI players that want to keep the whole clip
    /// in memory and seek freely; for long streams prefer
    /// [`read_samples`](Self::read_samples).
    pub fn decode_all(&mut self) -> io::Result<Vec<i16>> {
        match self {
            SoundDecoder::Acm(d) => d.decode_all().map_err(io::Error::from),
            SoundDecoder::Wav(d) => d.decode_all().map_err(io::Error::from),
        }
    }

    /// Rewind the decoder to the start of the stream by reopening the
    /// underlying datasource. The original `name` is preserved.
    pub fn reset(&mut self) -> io::Result<()> {
        match self {
            SoundDecoder::Acm(d) => d.reset().map_err(io::Error::from),
            SoundDecoder::Wav(d) => d.reset().map_err(io::Error::from),
        }
    }
}

impl From<AcmDecoder> for SoundDecoder {
    fn from(d: AcmDecoder) -> Self {
        SoundDecoder::Acm(d)
    }
}

impl From<WavDecoder> for SoundDecoder {
    fn from(d: WavDecoder) -> Self {
        SoundDecoder::Wav(d)
    }
}
