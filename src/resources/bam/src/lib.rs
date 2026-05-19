#![doc = include_str!("../readme.md")]

pub mod common;
mod exporter;
mod importer;

pub use exporter::BamExporter;
pub use importer::{BamImporter, detect_bam_type};
pub use importer::v1::{BamV1, BamV1Cycle, BamV1Frame, SharedRect};
pub use importer::v2::{BamV2, BamV2Cycle, BamV2DataBlock, BamV2Frame};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    BamC,  // BAM Compressed V1 (zlib-wrapped BAM V1)
    BamV1, // BAM V1
    BamV2, // BAM V2
}

pub const BAM_V1_SIGNATURE: &str = "BAM V1  ";
pub const BAM_V2_SIGNATURE: &str = "BAM V2  ";
pub const BAMC_SIGNATURE: &str = "BAMCV1  ";

impl Type {
    pub fn signature(&self) -> &'static str {
        match self {
            Type::BamV1 => BAM_V1_SIGNATURE,
            Type::BamV2 => BAM_V2_SIGNATURE,
            Type::BamC => BAMC_SIGNATURE,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Bam {
    V1(BamV1),
    V2(BamV2),
}
