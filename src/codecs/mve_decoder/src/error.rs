use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),
    #[error("Invalid MVE signature")]
    InvalidSignature,
    #[error("Video decode error: {0}")]
    VideoDecode(String),
    #[error("Audio decode error: {0}")]
    AudioDecode(String),
}

impl Into<std::io::Error> for Error {
    fn into(self) -> std::io::Error {
        match self {
            Error::Io(e) => e,
            _ => std::io::Error::new(std::io::ErrorKind::Other, self),
        }
    }
}