#![doc = include_str!("../readme.md")]
//!
//! Implementation outline:
//!
//! - Real ACM streams are decoded directly by
//!   [`infinitier_acm_decoder::AcmDecoder`] — the same decoder the WAVC
//!   path of the WAV resource uses internally.
//! - OGG/Vorbis streams that ship under the `.acm` extension (some
//!   Enhanced-Edition sound packs) are delegated to
//!   [`infinitier_wav_resource::WavDecoder`], which already wires up
//!   Symphonia's Ogg/Vorbis demuxer + decoder for the WAV crate's own
//!   "OGG-as-WAV" branch. We reuse that plumbing here instead of
//!   duplicating it.

use std::io;

use infinitier_datasource::{DataSource, Importer, ReadExt, Reader};
use infinitier_wav_resource::{WavDecoder, WavFormat};
use log::debug;

pub use infinitier_acm_decoder::*;

/// Which container the bytes turned out to live in. Surfaced to
/// callers so they can decide whether to pre-clip / resample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcmFormat {
    /// Interplay ACM (the historical Infinity-Engine sound codec).
    Acm,
    /// OGG/Vorbis stream shipped under the `.acm` extension. Some
    /// Enhanced-Edition sound packs do this.
    Ogg,
}

/// Streaming decoder for whichever flavour an `.acm` resource turned
/// out to be. The enum is the public type [`AcmImporter::import`]
/// returns — same idea as
/// [`infinitier_core::imported_resource::ImportedResource`], scoped
/// to ACM containers.
///
/// Use the inherent helpers below for the common path (read samples,
/// query metadata). Drop down to `match` when you need decoder-
/// specific knobs (e.g. forcing stereo / mono output on the ACM
/// branch via [`AcmDecoder::open`]'s `OutputChannels` argument).
#[derive(Debug)]
pub enum Acm {
    /// Genuine Interplay ACM stream.
    Acm(AcmDecoder),
    /// OGG/Vorbis stream under the `.acm` extension. Decoded via the
    /// WAV resource's OGG path so we don't ship a second copy of the
    /// Symphonia setup.
    Ogg(WavDecoder),
}

impl Acm {
    /// Resource label the caller passed at import time.
    pub fn name(&self) -> &str {
        match self {
            Acm::Acm(d) => d.name(),
            Acm::Ogg(d) => d.name(),
        }
    }

    /// Which container the bytes actually live in.
    pub fn format(&self) -> AcmFormat {
        match self {
            Acm::Acm(_) => AcmFormat::Acm,
            // `WavDecoder` only ever lands here via the OGG branch
            // below, but the assertion makes the invariant explicit
            // so a future change to the dispatch can't quietly turn a
            // PCM WAV into an "Ogg" return value.
            Acm::Ogg(d) => {
                debug_assert_eq!(
                    d.format(),
                    WavFormat::Ogg,
                    "Acm::Ogg holds a non-Ogg WavDecoder"
                );
                AcmFormat::Ogg
            }
        }
    }

    /// Channel count of the decoded PCM.
    pub fn channels(&self) -> u16 {
        match self {
            Acm::Acm(d) => d.info.channels as u16,
            Acm::Ogg(d) => d.info().channels,
        }
    }

    /// Sample rate (Hz) of the decoded PCM.
    pub fn sample_rate(&self) -> u32 {
        match self {
            Acm::Acm(d) => d.info.rate,
            Acm::Ogg(d) => d.info().sample_rate,
        }
    }

    /// Total `i16` values (samples × channels) the stream will
    /// produce. Eq to `channels × frames`.
    pub fn total_values(&self) -> u32 {
        match self {
            Acm::Acm(d) => d.info.total_values,
            Acm::Ogg(d) => d.info().total_values,
        }
    }

    /// Decode the next chunk of interleaved 16-bit PCM into `out`,
    /// returning the number of `i16` values written. `Ok(0)` only on
    /// natural end of stream.
    pub fn read_samples(&mut self, out: &mut [i16]) -> io::Result<usize> {
        match self {
            Acm::Acm(d) => d.read_samples(out).map_err(io::Error::from),
            Acm::Ogg(d) => d.read_samples(out).map_err(io::Error::from),
        }
    }

    /// Drain the entire stream into a single interleaved `Vec<i16>`.
    /// Convenience helper around [`Acm::read_samples`]; runs the same
    /// "fill a buffer until EOF" loop used everywhere else.
    pub fn decode_all(&mut self) -> io::Result<Vec<i16>> {
        let total = self.total_values() as usize;
        let mut samples = vec![0i16; total];
        let mut written = 0usize;
        while written < total {
            let n = self.read_samples(&mut samples[written..])?;
            if n == 0 {
                break;
            }
            written += n;
        }
        samples.truncate(written);
        Ok(samples)
    }

    /// Rewind to the start of the stream by reopening the underlying
    /// [`DataSource`].
    pub fn reset(&mut self) -> io::Result<()> {
        match self {
            Acm::Acm(d) => d.reset().map_err(io::Error::from),
            Acm::Ogg(d) => d.reset().map_err(io::Error::from),
        }
    }
}

/// An ACM sound resource importer.
///
/// Sniffs the first four bytes of the source: `OggS` selects the
/// OGG/Vorbis path (delegated to
/// [`infinitier_wav_resource::WavDecoder`], which the WAV resource
/// already uses for its own OGG-under-`.wav` branch), anything else
/// is handed to [`AcmDecoder`] which then validates the ACM /
/// `WAVC`-wrapped magic itself. `name` is the resource name (lowercase,
/// no extension — matches the workspace indexing convention) and
/// becomes the inner decoder's log label.
pub struct AcmImporter<'a> {
    pub name: &'a str,
}

impl Importer for AcmImporter<'_> {
    type T = Acm;

    fn import(&self, source: &DataSource) -> io::Result<Self::T> {
        // Peek four bytes without consuming the reader the inner
        // decoder will open separately from the `DataSource`.
        let mut reader: Reader<_> = source.reader()?;
        let (magic, _len): ([u8; 4], _) = reader.read_at_most_to_array()?;

        if &magic == b"OggS" {
            // OGG-under-ACM. Reuse the WAV resource's OGG path. The
            // `WavDecoder::open` dispatcher reads the four-byte magic
            // again itself — fine, the source is rewindable.
            let dec = WavDecoder::open(source, self.name)?;
            debug_assert_eq!(dec.format(), WavFormat::Ogg);
            debug!(
                "[{}] Opened OGG-as-ACM: channels={}, rate={}, total_values={}",
                self.name,
                dec.info().channels,
                dec.info().sample_rate,
                dec.info().total_values,
            );
            Ok(Acm::Ogg(dec))
        } else {
            let dec = AcmDecoder::open(source, OutputChannels::Original, self.name)?;
            debug!(
                "[{}] Opened ACM: channels={}, rate={}, total_values={}",
                self.name, dec.info.channels, dec.info.rate, dec.info.total_values,
            );
            Ok(Acm::Acm(dec))
        }
    }
}
