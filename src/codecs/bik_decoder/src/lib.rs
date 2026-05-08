#![doc = include_str!("../readme.md")]

pub mod audio;
pub mod binkb;
pub mod bitreader;
pub mod bundle;
pub mod container;
pub mod dct;
pub mod dct3;
pub mod dsp;
pub mod error;
pub mod fft;
pub mod rdft;
pub mod streaming;
pub mod tables;
pub mod video;
pub mod vlc;

pub use audio::{AudioDecoder, extract_audio_to_wav};
pub use streaming::{
    BikAudioChunk, BikFrame, BikOutputFormat, BikPixels, BikStreamingDecoder, BikVideoFrame,
};
pub use video::{BlockType, Plane, VideoDecoder, VideoFrame};

pub use container::{AudioFlags, AudioTrack, BikHeader, FrameEntry, parse_header};
pub use error::{BikError, BikResult};
