use std::io::BufRead;

use crate::{
    datasource::Reader,
    resource::bam::{Bam, BamImporter, Type},
};

/// A BAMC file importer
pub struct BamcParser;

impl BamcParser {
    /// Imports a BAMC file
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Bam> {
        let signature = reader.read_string(8)?;

        if !signature.eq(Type::BamC.signature()) {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        };

        let _uncompressed_size = reader.read_u32()?;

        let mut uncompressed_reader = reader.as_zip_reader().decode_all()?;

        BamImporter::from_reader(&mut uncompressed_reader)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        datasource::DataSource, resource::bam::bam_v1::BamV1Parser, test_utils::RESOURCES_DIR,
    };

    use super::*;

    #[test]
    fn test_parse_compressed_bam_should_fail_if_wrong_signature() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V1/01/1chan03B_decompressed.BAM"
        )));

        let mut reader = data.reader().unwrap();
        let res = BamcParser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_bam_v1_compressed_01() {
        let bam_from_decompressed = {
            let data = DataSource::new(Path::new(&format!(
                "{RESOURCES_DIR}/resources/BAM_V1/01/1chan03B_decompressed.BAM"
            )));

            let mut reader = data.reader().unwrap();
            BamV1Parser::import(&mut reader).unwrap()
        };

        let bam_from_compressed = {
            let data = DataSource::new(Path::new(&format!(
                "{RESOURCES_DIR}/resources/BAM_V1/01/1chan03B_compressed.BAM"
            )));

            let mut reader = data.reader().unwrap();
            BamcParser::import(&mut reader).unwrap()
        };

        assert_eq!(Bam::V1(bam_from_decompressed), bam_from_compressed);
    }

    #[test]
    fn test_parse_bam_v1_compressed_02() {
        let bam_from_decompressed = {
            let data = DataSource::new(Path::new(&format!(
                "{RESOURCES_DIR}/resources/BAM_V1/02/SPHEART_decompressed.BAM"
            )));

            let mut reader = data.reader().unwrap();
            BamV1Parser::import(&mut reader).unwrap()
        };

        let bam_from_compressed = {
            let data = DataSource::new(Path::new(&format!(
                "{RESOURCES_DIR}/resources/BAM_V1/02/SPHEART_compressed.BAM"
            )));

            let mut reader = data.reader().unwrap();
            BamcParser::import(&mut reader).unwrap()
        };

        assert_eq!(Bam::V1(bam_from_decompressed), bam_from_compressed);
    }
}
