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

impl From<Error> for std::io::Error {
    fn from(val: Error) -> Self {
        match val {
            Error::Io(e) => e,
            _ => std::io::Error::other(val),
        }
    }
}
