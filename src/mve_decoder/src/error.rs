use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid MVE signature")]
    InvalidSignature,
    #[error("Video decode error: {0}")]
    VideoDecode(String),
    #[error("Audio decode error: {0}")]
    AudioDecode(String),
}
