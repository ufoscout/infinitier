//! Unified streaming video decoder over the engine's three cutscene
//! formats: Interplay's MVE, RAD's Bink Video v1 (BIKi), and Beamdog's
//! WBM (= WebM, VP8 + Vorbis).
//!
//! Mirrors the [`SoundDecoder`](crate::sound::SoundDecoder) pattern —
//! [`MovieDecoder`] is an enum that wraps the per-format streaming
//! decoder (each codec crate provides its own; we just dispatch) and
//! exposes a single `next_frame` API yielding the format-neutral
//! [`MovieFrame`]. UI players (e.g. the explorer's `MovieViewer`)
//! consume one `MovieDecoder` regardless of which container the
//! resource ships under.
//!
//! All three formats appear in the wild under varied extensions — IWD2
//! ships its Bink cutscenes as `.MVE`, the Enhanced Editions ship VP8
//! cutscenes as `.WBM` — so [`MovieSource::open`] sniffs the magic and
//! dispatches accordingly.

use std::io::{self, Read};

use infinitier_bik_decoder::{BikError, BikOutputFormat, BikPixels, BikStreamingDecoder};
use infinitier_datasource::{DataSource, DataTrait, Reader};
use infinitier_mve_decoder::{Error as MveError, MveDecoder};
use infinitier_wbm_decoder::{
    EBML_MAGIC, WbmError, WbmOutputFormat, WbmPixels, WbmStreamingDecoder,
};

mod frame;

pub use frame::{MovieAudioChunk, MovieFrame, MovieVideoFrame};

/// Type alias for the boxed-trait-object MVE decoder.
pub type StreamingMveDecoder = MveDecoder<Box<dyn DataTrait>>;
/// Type alias for the boxed-trait-object Bink streaming decoder. Bink's
/// per-frame index uses `Read + Seek` directly (no `Reader` wrapper)
/// because it doesn't need encoding-aware string parsing.
pub type StreamingBikDecoder = BikStreamingDecoder<Reader<Box<dyn DataTrait>>>;
/// Type alias for the boxed-trait-object WBM streaming decoder. Like
/// BIK, the Matroska demuxer underneath only needs `Read + Seek`.
pub type StreamingWbmDecoder = WbmStreamingDecoder<Reader<Box<dyn DataTrait>>>;

/// Container of the underlying movie stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovieFormat {
    /// Interplay MVE (`Interplay MVE File…` magic).
    Mve,
    /// RAD Bink Video v1 (`BIKi` / `BIKb` / `BIKf` / `BIKg` / `BIKh` /
    /// `BIKk` magic).
    Bik,
    /// Beamdog WBM (= WebM, VP8 + Vorbis in an EBML/Matroska container).
    Wbm,
}

impl std::fmt::Display for MovieFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MovieFormat::Mve => f.write_str("MVE"),
            MovieFormat::Bik => f.write_str("BIK"),
            MovieFormat::Wbm => f.write_str("WBM"),
        }
    }
}

/// Stream-level metadata, lifted from whichever container backs the
/// decoder.
#[derive(Debug, Clone, Copy)]
pub struct MovieInfo {
    pub width: u16,
    pub height: u16,
    /// Display duration of one frame, in microseconds.
    pub frame_duration_us: u32,
    /// Total stream duration in microseconds.
    pub total_duration_us: u64,
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
    /// "Interplay MVE File…" magic.
    pub fn open(&self) -> Result<MovieDecoder, MovieOpenError> {
        let mut probe_reader = self.datasource.reader()?;
        let mut probe = [0u8; 4];
        probe_reader.read_exact(&mut probe)?;
        let format = MovieFormat::from_magic(&probe)?;
        // Drop the probe reader so the underlying file handle is
        // released before we open a fresh one positioned at byte 0.
        drop(probe_reader);

        match format {
            MovieFormat::Mve => {
                let reader = self.datasource.reader()?;
                Ok(MovieDecoder::Mve(Box::new(MveDecoder::new(
                    reader, &self.name,
                )?)))
            }
            MovieFormat::Bik => {
                let inner = self.datasource.reader()?;
                let dec = BikStreamingDecoder::new(inner, &self.name)?
                    .with_output_format(BikOutputFormat::Rgba);
                Ok(MovieDecoder::Bik(Box::new(dec)))
            }
            MovieFormat::Wbm => {
                let inner = self.datasource.reader()?;
                let dec = WbmStreamingDecoder::new(inner, &self.name)?
                    .with_output_format(WbmOutputFormat::Rgba);
                Ok(MovieDecoder::Wbm(Box::new(dec)))
            }
        }
    }
}

impl MovieFormat {
    /// Match the first four bytes of a stream against the known
    /// cutscene magics. Returns [`MovieOpenError::UnknownFormat`] if
    /// none matches.
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
        // WBM: EBML header magic. Matroska / WebM files all start with
        // these four bytes regardless of the inner DocType.
        if magic.len() >= 4 && magic[..4] == EBML_MAGIC {
            return Ok(MovieFormat::Wbm);
        }
        let mut head = [0u8; 4];
        head.copy_from_slice(&magic[..magic.len().min(4)]);
        Err(MovieOpenError::UnknownFormat(head))
    }
}

/// Streaming movie decoder. Pull frames with [`Self::next_frame`] until
/// it returns `Ok(None)`.
pub enum MovieDecoder {
    Mve(Box<StreamingMveDecoder>),
    Bik(Box<StreamingBikDecoder>),
    Wbm(Box<StreamingWbmDecoder>),
}

impl MovieDecoder {
    /// Container format backing this decoder.
    pub fn format(&self) -> MovieFormat {
        match self {
            MovieDecoder::Mve(_) => MovieFormat::Mve,
            MovieDecoder::Bik(_) => MovieFormat::Bik,
            MovieDecoder::Wbm(_) => MovieFormat::Wbm,
        }
    }

    /// Stream-level metadata. For MVE the `frame_duration_us` lives
    /// inside the first frame's timer chunk, so it is `0` here until
    /// the first [`Self::next_frame`] has returned; for BIK and WBM
    /// it is known at construction time (BIK reads it from the
    /// header, WBM from the Matroska track's `DefaultDuration`).
    pub fn info(&self) -> MovieInfo {
        match self {
            MovieDecoder::Mve(d) => MovieInfo {
                width: d.width(),
                height: d.height(),
                frame_duration_us: d.frame_duration_us(),
                total_duration_us: d.total_duration_us(),
            },
            MovieDecoder::Bik(d) => MovieInfo {
                width: d.width() as u16,
                height: d.height() as u16,
                frame_duration_us: d.frame_duration_us(),
                total_duration_us: d.total_duration_us(),
            },
            MovieDecoder::Wbm(d) => MovieInfo {
                width: d.width() as u16,
                height: d.height() as u16,
                frame_duration_us: d.frame_duration_us(),
                total_duration_us: d.total_duration_us(),
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
            MovieDecoder::Wbm(d) => match d.next_frame()? {
                Some(frame) => {
                    let pixels = match frame.video.pixels {
                        WbmPixels::Rgba(p) => p,
                        WbmPixels::Yuv(_) => {
                            return Err(MovieDecodeError::UnexpectedYuvFromWbm);
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
    #[error("wbm error: {0}")]
    Wbm(#[from] WbmError),
    #[error("unknown movie format: magic = {0:?}")]
    UnknownFormat([u8; 4]),
}

impl From<MovieOpenError> for io::Error {
    fn from(val: MovieOpenError) -> Self {
        match val {
            MovieOpenError::Io(e) => e,
            _ => io::Error::other(val),
        }
    }
}

/// Errors returned from [`MovieDecoder::next_frame`]. Wraps the
/// per-format decoder errors so callers can `?` through.
#[derive(Debug, thiserror::Error)]
pub enum MovieDecodeError {
    #[error("mve decode error: {0}")]
    Mve(#[from] MveError),
    #[error("bik decode error: {0}")]
    Bik(#[from] BikError),
    #[error("wbm decode error: {0}")]
    Wbm(#[from] WbmError),
    /// The Bink decoder returned YUV pixels from a [`MovieDecoder`]
    /// that should have been configured for RGBA. Indicates a bug in
    /// the dispatcher (i.e. someone constructed a `MovieDecoder::Bik`
    /// without calling `with_output_format(BikOutputFormat::Rgba)`).
    #[error("internal: BIK decoder delivered YUV pixels to MovieDecoder (expected RGBA)")]
    UnexpectedYuvFromBik,
    /// Same as `UnexpectedYuvFromBik`, for the WBM dispatcher arm.
    #[error("internal: WBM decoder delivered YUV pixels to MovieDecoder (expected RGBA)")]
    UnexpectedYuvFromWbm,
}

impl From<MovieDecodeError> for io::Error {
    fn from(val: MovieDecodeError) -> Self {
        match val {
            MovieDecodeError::Mve(e) => e.into(),
            MovieDecodeError::Bik(e) => e.into(),
            MovieDecodeError::Wbm(e) => e.into(),
            MovieDecodeError::UnexpectedYuvFromBik | MovieDecodeError::UnexpectedYuvFromWbm => {
                io::Error::other(val)
            }
        }
    }
}
