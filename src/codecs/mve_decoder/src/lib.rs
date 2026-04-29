#![doc = include_str!("../readme.md")]

mod audio;
mod decoder;
mod error;
mod video;

pub use decoder::MveDecoder;
pub use error::Error;

/// Pixel format of a decoded video frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    /// 8-bit paletted
    Palette8,
    /// 16-bit RGB555
    Rgb555,
}

/// A single decoded video frame (RGBA pixels).
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u16,
    pub height: u16,
    /// Row-major RGBA pixels, 4 bytes per pixel.
    pub pixels: Vec<u8>,
    /// How long to display this frame, in microseconds.
    pub duration_us: u32,
}

/// PCM audio data accompanying a video frame.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub channels: u8,
    pub sample_rate: u32,
    /// Interleaved signed 16-bit PCM samples.
    pub samples: Vec<i16>,
}

/// A complete decoded frame: video + any associated audio.
#[derive(Debug)]
pub struct MveFrame {
    pub video: VideoFrame,
    pub audio: Vec<AudioChunk>,
}
