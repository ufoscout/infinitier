//! Wrapper around an [`infinitier_mve_decoder::MveDecoder`] source.
//!
//! `MveDecoder` doesn't have a built-in `reset()` (unlike the audio
//! decoders), so a streaming UI player needs to re-open it from scratch
//! whenever the user hits Stop. [`MovieSource`] keeps the underlying
//! [`DataSource`] and the caller-supplied `name` together so the viewer
//! can produce as many fresh decoders as it needs from a single import.

use std::io;

use infinitier_datasource::{DataSource, DataTrait, Reader};
use infinitier_mve_decoder::{Error as MveError, MveDecoder};

/// Type alias for the boxed-trait-object decoder used by the viewer
/// (and by the bundled mve player). Pulled out so call sites don't have
/// to spell `MveDecoder<Reader<Box<dyn DataTrait>>>` every time.
pub type StreamingMveDecoder = MveDecoder<Box<dyn DataTrait>>;

/// A reusable handle to an MVE resource. Holds the [`DataSource`] and a
/// caller-supplied label and exposes [`MovieSource::open`] to spin up a
/// fresh decoder positioned at the start of the stream.
#[derive(Debug, Clone)]
pub struct MovieSource {
    pub datasource: DataSource,
    pub name: String,
}

impl MovieSource {
    pub fn new(datasource: DataSource, name: impl Into<String>) -> Self {
        Self {
            datasource,
            name: name.into(),
        }
    }

    /// Open a fresh decoder. Called once on import to pull metadata
    /// (width, height, frame duration), and again on every Play press
    /// when the viewer is restarting playback from the beginning.
    pub fn open(&self) -> Result<StreamingMveDecoder, MovieOpenError> {
        let inner = self.datasource.reader()?;
        let reader = Reader {
            data: inner.data,
            charset: inner.charset,
        };
        Ok(MveDecoder::new(reader, &self.name)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MovieOpenError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("mve error: {0}")]
    Mve(#[from] MveError),
}
