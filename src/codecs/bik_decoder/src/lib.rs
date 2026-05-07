//! Pure-Rust decoder for Bink Video v1 (BIKi).
//!
//! Crate scope:
//! - **Container parser** ([`container::parse_header`]) — full BIKi/BIKb/BIKf
//!   header + frame index + audio-track-table parsing.
//! - **Video decoder** (TODO, phases 2-6) — port of FFmpeg's
//!   `libavcodec/bink.c`.
//! - **Audio decoder** (TODO, phase 7) — port of FFmpeg's `binkaudio_dct`
//!   path (the variant IWD2 uses).
//!
//! The split between phases lives behind module boundaries so the container
//! parser is usable on its own (e.g. for the IWD2 corpus tests that just
//! enumerate frame counts and audio metadata).

pub mod audio;
pub mod binkb;
pub mod bitreader;
pub mod bundle;
pub mod container;
pub mod dct;
pub mod dsp;
pub mod error;
pub mod fft;
pub mod rdft;
pub mod tables;
pub mod vlc;
pub mod video;

pub use audio::AudioDecoder;
pub use video::{BlockType, Plane, VideoDecoder, VideoFrame};

pub use container::{AudioFlags, AudioTrack, BikHeader, FrameEntry, parse_header};
pub use error::{BikError, BikResult};
