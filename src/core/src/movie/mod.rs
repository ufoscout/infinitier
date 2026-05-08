//! Unified streaming video decoder over the engine's two cutscene
//! formats: Interplay's MVE and RAD's Bink Video v1 (BIKi).
//!
//! Mirrors the [`SoundDecoder`](crate::sound::SoundDecoder) pattern —
//! [`MovieDecoder`] is an enum that wraps the per-format streaming
//! decoder (each codec crate provides its own; we just dispatch) and
//! exposes a single `next_frame` API yielding the format-neutral
//! [`MovieFrame`]. UI players (e.g. the explorer's `MovieViewer`)
//! consume one `MovieDecoder` regardless of which container the
//! resource ships under.
//!
//! Both formats appear in the wild under the `.MVE` extension — IWD2
//! ships its Bink cutscenes that way — so [`MovieSource::open`]
//! sniffs the magic and dispatches accordingly.

use std::io::{self, Read};

use infinitier_bik_decoder::{
    BikError, BikOutputFormat, BikPixels, BikStreamingDecoder,
};
use infinitier_datasource::{DataSource, DataTrait, Reader};
use infinitier_mve_decoder::{Error as MveError, MveDecoder};

mod frame;

pub use frame::{MovieAudioChunk, MovieFrame, MovieVideoFrame};

/// Type alias for the boxed-trait-object MVE decoder.
pub type StreamingMveDecoder = MveDecoder<Box<dyn DataTrait>>;
/// Type alias for the boxed-trait-object Bink streaming decoder. Bink's
/// per-frame index uses `Read + Seek` directly (no `Reader` wrapper)
/// because it doesn't need encoding-aware string parsing.
pub type StreamingBikDecoder = BikStreamingDecoder<Box<dyn DataTrait>>;

/// Container of the underlying movie stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovieFormat {
    /// Interplay MVE (`Interplay MVE File…` magic).
    Mve,
    /// RAD Bink Video v1 (`BIKi` / `BIKb` / `BIKf` / `BIKg` / `BIKh` /
    /// `BIKk` magic).
    Bik,
}

impl std::fmt::Display for MovieFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MovieFormat::Mve => f.write_str("MVE"),
            MovieFormat::Bik => f.write_str("BIK"),
        }
    }
}

/// Stream-level metadata, lifted from whichever container backs the
/// decoder.
#[derive(Debug, Clone, Copy)]
pub struct MovieInfo {
    pub width: u16,
    pub height: u16,
    /// Display duration of one frame, in microseconds. For MVE this is
    /// the global timer set by the file's first timer chunk (so it is
    /// `0` until the first frame has been pulled); for BIK it is
    /// `fps_den * 1_000_000 / fps_num` — known up front from the
    /// header.
    pub frame_duration_us: u32,
}

/// A reusable handle to a movie resource. Holds the [`DataSource`] and
/// a caller-supplied label and exposes [`MovieSource::open`] to spin up
/// a fresh [`MovieDecoder`] positioned at the start of the stream.
///
/// Neither codec's streaming decoder has a built-in `reset()`, so a
/// streaming UI player needs to re-open the source whenever the user
/// hits Stop. `MovieSource` keeps the `DataSource` (which is cheap to
/// `clone`) around for exactly that.
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

    /// Open a fresh streaming decoder. Sniffs the first four bytes to
    /// pick the format — Bink's "BIK*" magic vs. MVE's
    /// "Interplay MVE File…" magic — then opens a fresh reader so the
    /// decoder sees the full stream from byte 0. The Bink branch is
    /// configured to deliver RGBA pixels (a `MovieFrame` is RGBA by
    /// definition); callers that need raw YUV planes should use
    /// `infinitier_bik_decoder::BikStreamingDecoder` directly.
    pub fn open(&self) -> Result<MovieDecoder, MovieOpenError> {
        let mut probe_reader = self.datasource.reader()?;
        let mut probe = [0u8; 4];
        probe_reader.data.read_exact(&mut probe)?;
        let format = MovieFormat::from_magic(&probe)?;
        // Drop the probe reader so the underlying file handle is
        // released before we open a fresh one positioned at byte 0.
        drop(probe_reader);

        match format {
            MovieFormat::Mve => {
                let inner = self.datasource.reader()?;
                let reader = Reader {
                    data: inner.data,
                    charset: inner.charset,
                };
                Ok(MovieDecoder::Mve(Box::new(MveDecoder::new(
                    reader, &self.name,
                )?)))
            }
            MovieFormat::Bik => {
                let inner = self.datasource.reader()?;
                let dec = BikStreamingDecoder::new(inner.data, &self.name)?
                    .with_output_format(BikOutputFormat::Rgba);
                Ok(MovieDecoder::Bik(Box::new(dec)))
            }
        }
    }
}

impl MovieFormat {
    /// Match the first four bytes of a stream against the known
    /// cutscene magics. Returns [`MovieOpenError::UnknownFormat`] if
    /// neither matches.
    pub fn from_magic(magic: &[u8]) -> Result<Self, MovieOpenError> {
        // MVE: `Interplay MVE File…`. The leading "Inte" tag is unique
        // among the Infinity Engine resource set, so checking the first
        // four bytes is enough to disambiguate from any Bink tag.
        if magic.len() >= 4 && &magic[..4] == b"Inte" {
            return Ok(MovieFormat::Mve);
        }
        // BIK: any of the six recognised codec tags.
        if magic.len() >= 4
            && magic[0] == b'B'
            && magic[1] == b'I'
            && magic[2] == b'K'
            && matches!(magic[3], b'i' | b'b' | b'f' | b'g' | b'h' | b'k')
        {
            return Ok(MovieFormat::Bik);
        }
        let mut head = [0u8; 4];
        head.copy_from_slice(&magic[..magic.len().min(4)]);
        Err(MovieOpenError::UnknownFormat(head))
    }
}

/// Streaming movie decoder. Pull frames with [`Self::next_frame`] until
/// it returns `Ok(None)`.
///
/// Both variants are boxed because each decoder carries ~½–3 KB of
/// internal state (palette / trig tables / per-codec scratch); keeping
/// the enum at one machine word per variant means matching, moving and
/// returning a `MovieDecoder` is cheap.
pub enum MovieDecoder {
    Mve(Box<StreamingMveDecoder>),
    Bik(Box<StreamingBikDecoder>),
}

impl MovieDecoder {
    /// Container format backing this decoder.
    pub fn format(&self) -> MovieFormat {
        match self {
            MovieDecoder::Mve(_) => MovieFormat::Mve,
            MovieDecoder::Bik(_) => MovieFormat::Bik,
        }
    }

    /// Stream-level metadata. For MVE the `frame_duration_us` lives
    /// inside the first frame's timer chunk, so it is `0` here until
    /// the first [`Self::next_frame`] has returned; for BIK it is
    /// known from the header at construction time.
    pub fn info(&self) -> MovieInfo {
        match self {
            MovieDecoder::Mve(d) => MovieInfo {
                width: d.width(),
                height: d.height(),
                frame_duration_us: d.frame_duration_us(),
            },
            MovieDecoder::Bik(d) => MovieInfo {
                width: d.width() as u16,
                height: d.height() as u16,
                frame_duration_us: d.frame_duration_us(),
            },
        }
    }

    /// Decode and return the next complete video frame plus any audio
    /// chunks accumulated since the previous frame. Returns `Ok(None)`
    /// at the natural end of the stream.
    pub fn next_frame(&mut self) -> Result<Option<MovieFrame>, MovieDecodeError> {
        match self {
            MovieDecoder::Mve(d) => match d.next_frame()? {
                Some(frame) => Ok(Some(MovieFrame {
                    video: MovieVideoFrame {
                        width: frame.video.width,
                        height: frame.video.height,
                        pixels: frame.video.pixels,
                        duration_us: frame.video.duration_us,
                    },
                    audio: frame
                        .audio
                        .into_iter()
                        .map(|c| MovieAudioChunk {
                            channels: c.channels,
                            sample_rate: c.sample_rate,
                            samples: c.samples,
                        })
                        .collect(),
                })),
                None => Ok(None),
            },
            MovieDecoder::Bik(d) => match d.next_frame()? {
                Some(frame) => {
                    // `MovieSource::open` always configures the BIK
                    // decoder for RGBA output, so this match is total
                    // in practice. We still check at runtime so a
                    // future refactor that forgets the configuration
                    // step trips here loudly.
                    let pixels = match frame.video.pixels {
                        BikPixels::Rgba(p) => p,
                        BikPixels::Yuv(_) => {
                            return Err(MovieDecodeError::UnexpectedYuvFromBik);
                        }
                    };
                    Ok(Some(MovieFrame {
                        video: MovieVideoFrame {
                            width: frame.video.width as u16,
                            height: frame.video.height as u16,
                            pixels,
                            duration_us: frame.video.duration_us,
                        },
                        audio: frame
                            .audio
                            .into_iter()
                            .map(|c| MovieAudioChunk {
                                channels: c.channels,
                                sample_rate: c.sample_rate,
                                samples: c.samples,
                            })
                            .collect(),
                    }))
                }
                None => Ok(None),
            },
        }
    }
}

impl std::fmt::Debug for MovieDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieDecoder")
            .field("format", &self.format())
            .field("info", &self.info())
            .finish()
    }
}

/// Errors returned from [`MovieSource::open`].
#[derive(Debug, thiserror::Error)]
pub enum MovieOpenError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("mve error: {0}")]
    Mve(#[from] MveError),
    #[error("bik error: {0}")]
    Bik(#[from] BikError),
    #[error("unknown movie format: magic = {0:?}")]
    UnknownFormat([u8; 4]),
}

/// Errors returned from [`MovieDecoder::next_frame`]. Wraps the
/// per-format decoder errors so callers can `?` through.
#[derive(Debug, thiserror::Error)]
pub enum MovieDecodeError {
    #[error("mve decode error: {0}")]
    Mve(#[from] MveError),
    #[error("bik decode error: {0}")]
    Bik(#[from] BikError),
    /// The Bink decoder returned YUV pixels from a [`MovieDecoder`]
    /// that should have been configured for RGBA. Indicates a bug in
    /// the dispatcher (i.e. someone constructed a `MovieDecoder::Bik`
    /// without calling `with_output_format(BikOutputFormat::Rgba)`).
    #[error("internal: BIK decoder delivered YUV pixels to MovieDecoder (expected RGBA)")]
    UnexpectedYuvFromBik,
}
