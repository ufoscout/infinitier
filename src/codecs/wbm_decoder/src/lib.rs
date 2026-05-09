#![doc = include_str!("../readme.md")]

mod error;
mod streaming;
mod vorbis;
mod yuv;

pub use error::{WbmError, WbmResult};
pub use streaming::{
    WbmAudioChunk, WbmFrame, WbmOutputFormat, WbmPixels, WbmStreamingDecoder, WbmVideoFrame,
    WbmYuvFrame,
};

/// EBML magic — first four bytes of every Matroska / WebM (and thus
/// every WBM) file. Use to disambiguate from the other in-tree
/// cutscene formats (MVE, BIK*).
pub const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];
