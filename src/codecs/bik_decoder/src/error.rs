use thiserror::Error;

/// Errors that can occur while decoding a Bink stream.
#[derive(Debug, Error)]
pub enum BikError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not a Bink Video v1 file: expected one of \"BIKi\"/\"BIKb\"/\"BIKf\", got {0:?}")]
    BadSignature([u8; 4]),

    #[error(
        "rejected header: {field} = {value} (limit: {limit}). Possibly corrupt or KB2 (Bink 2)?"
    )]
    InvalidHeader {
        field: &'static str,
        value: u64,
        limit: u64,
    },

    #[error("invalid frame index entry {index}: next offset {next} <= current offset {cur}")]
    InvalidFrameIndex {
        index: usize,
        cur: u32,
        next: u32,
    },

    #[error("unsupported codec variant: {0}")]
    Unsupported(&'static str),

    #[error("bitstream truncated at byte {pos} (expected at least {needed} more bytes)")]
    Truncated { pos: usize, needed: usize },

    #[error("malformed bitstream: {0}")]
    Malformed(&'static str),
}

pub type BikResult<T> = std::result::Result<T, BikError>;
