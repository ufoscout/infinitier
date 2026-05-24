use std::io::{BufRead, Read, Seek};

use infinitier_datasource::{Importer, Reader, SeekExt};
use log::{debug, error};

use crate::{BAM_V1_SIGNATURE, BAM_V2_SIGNATURE, BAMC_SIGNATURE, Bam, Type};

pub(crate) mod bamc;
pub(crate) mod v1;
pub(crate) mod v2;

use bamc::BamcParser;
use v1::BamV1Parser;
use v2::BamV2Parser;

/// A BAM file importer
pub struct BamImporter<'a> {
    pub name: &'a str,
}

impl Importer for BamImporter<'_> {
    type T = Bam;

    fn import(&self, source: &infinitier_datasource::DataSource) -> std::io::Result<Self::T> {
        let reader = &mut source.reader()?;
        let bam = BamImporter::from_reader(reader)?;
        debug!("Loaded {} [BAM]", self.name);
        Ok(bam)
    }
}

impl BamImporter<'_> {
    /// Imports a BAM file
    pub fn from_reader<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<Bam> {
        let position = reader.position()?;

        match detect_bam_type(reader)? {
            Type::BamV1 => {
                reader.set_position(position)?;
                BamV1Parser::import(reader).map(Bam::V1)
            }
            Type::BamV2 => {
                reader.set_position(position)?;
                BamV2Parser::import(reader).map(Bam::V2)
            }
            Type::BamC => {
                reader.set_position(position)?;
                BamcParser::import(reader)
            }
        }
    }
}

/// Detects the type of a BAM file
pub fn detect_bam_type<R: Read>(reader: &mut Reader<R>) -> std::io::Result<Type> {
    let value = reader.read_string(8)?;

    match value.as_str() {
        BAM_V1_SIGNATURE => Ok(Type::BamV1),
        BAM_V2_SIGNATURE => Ok(Type::BamV2),
        BAMC_SIGNATURE => Ok(Type::BamC),
        val => {
            error!("Unsupported BAM file signature: {:?}", val);
            Err(std::io::Error::other(format!(
                "Unsupported BAM file: {}",
                val
            )))
        }
    }
}

#[cfg(test)]
mod tests {

    use infinitier_datasource::DataSource;
    use infinitier_test_utils::get_assets_path;

    use super::*;

    #[test]
    fn test_detect_bam_v1_type() {
        let data = DataSource::new(get_assets_path().join("BAM/V1/01/1chan03B_decompressed.BAM"));

        assert_eq!(
            detect_bam_type(&mut data.reader().unwrap()).unwrap(),
            Type::BamV1
        );
    }

    #[test]
    fn test_detect_bam_v2_type() {
        let data = DataSource::new(get_assets_path().join("BAM/V2/1CHELM03.BAM"));

        assert_eq!(
            detect_bam_type(&mut data.reader().unwrap()).unwrap(),
            Type::BamV2
        );
    }

    #[test]
    fn test_detect_bamc_type() {
        let data = DataSource::new(get_assets_path().join("BAM/V1/01/1chan03B_compressed.BAM"));

        assert_eq!(
            detect_bam_type(&mut data.reader().unwrap()).unwrap(),
            Type::BamC
        );
    }
}
