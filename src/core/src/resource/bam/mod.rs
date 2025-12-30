use std::io::Read;

use crate::{
    datasource::{Importer, Reader},
    resource::bam::{bam_v1::BamV1Parser, bam_v2::BamV2Parser, bamc::BamcParser},
};

pub use bam_v1::BamV1;

mod bam_v1;
mod bam_v2;
mod bamc;

/// A BAM file importer
pub struct BamImporter {}

impl Importer for BamImporter {
    type T = Bam;

    fn import(source: &crate::datasource::DataSource) -> std::io::Result<Self::T> {
        let reader = &mut source.reader()?;
        let position = reader.position()?;

        match detect_bam_type(reader)? {
            Type::BamV1 => {
                reader.set_position(position)?;
                BamV1Parser::import(reader).map(|bam| Bam::V1(bam))
            }
            Type::BamV2 => {
                reader.set_position(position)?;
                BamV2Parser::import(reader)
            }
            Type::BamC => {
                reader.set_position(position)?;
                BamcParser::import(reader)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    BamC,  // BAM Compressed V1
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
}

/// Detects the type of a BAM file
fn detect_bam_type<R: Read>(reader: &mut Reader<R>) -> std::io::Result<Type> {
    let value = reader.read_string(8)?;

    match value.as_str() {
        BAM_V1_SIGNATURE => Ok(Type::BamV1),
        BAM_V2_SIGNATURE => Ok(Type::BamV2),
        BAMC_SIGNATURE => Ok(Type::BamC),
        val => Err(std::io::Error::other(format!(
            "Unsupported BAM file: {}",
            val
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};

    use super::*;

    #[test]
    fn test_detect_bam_v1_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V1/1chan03B_decompressed.BAM"
        )));

        assert_eq!(
            detect_bam_type(&mut data.reader().unwrap()).unwrap(),
            Type::BamV1
        );
    }

    #[test]
    fn test_detect_bam_v2_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V2/SPHEART.BAM"
        )));

        assert_eq!(
            detect_bam_type(&mut data.reader().unwrap()).unwrap(),
            Type::BamV2
        );
    }

    #[test]
    fn test_detect_bamc_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V1/1chan03B_compressed.BAM"
        )));

        assert_eq!(
            detect_bam_type(&mut data.reader().unwrap()).unwrap(),
            Type::BamC
        );
    }
}
