use std::io;

/// Result alias used throughout the crate.
pub type WbmResult<T> = Result<T, WbmError>;

/// All failure modes of the WBM decode pipeline.
///
/// Wraps the three underlying crate errors so callers can `?`-thread
/// through and (when needed) downcast to the specific layer that
/// failed.
#[derive(Debug, thiserror::Error)]
pub enum WbmError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("matroska demux error: {0}")]
    Matroska(#[from] matroska_demuxer::DemuxError),

    #[error("vp8 decode error: {0}")]
    Vp8(oxideav_core::Error),

    #[error("vorbis header error: {0}")]
    VorbisHeader(#[from] lewton::header::HeaderReadError),

    #[error("vorbis decode error: {0}")]
    VorbisDecode(#[from] lewton::audio::AudioReadError),

    #[error("missing VP8 video track in WBM container")]
    NoVideoTrack,

    #[error("video track has no PixelWidth/PixelHeight")]
    MissingDimensions,

    #[error("malformed Vorbis CodecPrivate (Xiph lacing): {0}")]
    BadXiphLacing(String),
}

impl From<oxideav_core::Error> for WbmError {
    fn from(e: oxideav_core::Error) -> Self {
        WbmError::Vp8(e)
    }
}

impl From<WbmError> for io::Error {
    fn from(e: WbmError) -> io::Error {
        match e {
            WbmError::Io(io) => io,
            other => io::Error::other(other),
        }
    }
}
