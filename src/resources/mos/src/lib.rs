#![doc = include_str!("../readme.md")]

pub mod common;
mod importer;

pub use importer::v1::{MosV1, MosV1Block};
pub use importer::v2::{MosV2, MosV2DataBlock};
pub use importer::{MosImporter, detect_mos_type};

/// Recognized MOS resource variants. Mirrors NearInfinity's
/// `MosDecoder.Type` (ignoring the `INVALID` sentinel — wrong signatures
/// surface as `io::Error` instead).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    /// Uncompressed MOS V1 (paletted, 64×64 block grid).
    MosV1,
    /// MOS V2 (PVRZ-backed atlas, mirrors BAM V2's structure).
    MosV2,
    /// MOSC V1 (zlib-compressed BAM-style wrapper around a MOS V1 payload).
    Mosc,
}

pub const MOS_V1_SIGNATURE: &[u8; 8] = b"MOS V1  ";
pub const MOS_V2_SIGNATURE: &[u8; 8] = b"MOS V2  ";
pub const MOSC_V1_SIGNATURE: &[u8; 8] = b"MOSCV1  ";

impl Type {
    pub fn signature(&self) -> &'static [u8; 8] {
        match self {
            Type::MosV1 => MOS_V1_SIGNATURE,
            Type::MosV2 => MOS_V2_SIGNATURE,
            Type::Mosc => MOSC_V1_SIGNATURE,
        }
    }
}

/// A parsed MOS file.
///
/// The `MosImporter` collapses MOS V1 and MOSC into the same in-memory
/// variant (`Mos::V1`) — the only externally visible difference is the
/// `r#type` tag, which preserves the original wrapper so that a future
/// exporter can round-trip back to MOSC when desired (same pattern as
/// [`infinitier_bam_resource::Bam`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Mos {
    V1(MosV1),
    V2(MosV2),
}
