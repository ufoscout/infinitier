/// One decoded video frame, RGBA8 pixels packed `width * height * 4`
/// bytes row-major.
#[derive(Debug, Clone)]
pub struct MovieVideoFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
    /// How long to display this frame, in microseconds. For BIK this is
    /// constant across the stream (derived from the header's fps); for
    /// MVE it is whatever the per-frame timer chunk reported.
    pub duration_us: u32,
}

/// PCM audio data accompanying a video frame.
#[derive(Debug, Clone)]
pub struct MovieAudioChunk {
    pub channels: u8,
    pub sample_rate: u32,
    /// Interleaved signed 16-bit PCM samples.
    pub samples: Vec<i16>,
}

/// A complete decoded frame: video + any associated audio chunks.
/// MVE may emit zero or more chunks per frame; BIK emits at most one.
#[derive(Debug, Clone)]
pub struct MovieFrame {
    pub video: MovieVideoFrame,
    pub audio: Vec<MovieAudioChunk>,
}
