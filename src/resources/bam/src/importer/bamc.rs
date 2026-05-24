use std::io::BufRead;

use infinitier_datasource::{ReadExt, Reader};
use log::{debug, error};

use crate::{Bam, Type};

use super::BamImporter;

/// A BAMC file importer
pub struct BamcParser;

impl BamcParser {
    /// Imports a BAMC file.
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Bam> {
        let signature = reader.read_string(8)?;

        if !signature.eq(Type::BamC.signature()) {
            error!("Not a BAMC file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        };

        let _uncompressed_size = reader.read_u32()?;

        debug!("Decompressing BAMC data");
        let mut uncompressed_reader = reader.as_zip_reader().decode_all()?;

        let inner = BamImporter::from_reader(&mut uncompressed_reader)?;
        Ok(match inner {
            Bam::V1(mut v1) => {
                v1.r#type = Type::BamC;
                Bam::V1(v1)
            }
            Bam::V2(v2) => Bam::V2(v2),
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::get_assets_path;

    use super::super::v1::BamV1Parser;

    use super::*;

    #[test]
    fn test_parse_compressed_bam_should_fail_if_wrong_signature() {
        let data = DataSource::new(get_assets_path().join("BAM/V1/01/1chan03B_decompressed.BAM"));

        let mut reader = data.reader().unwrap();
        let res = BamcParser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_bam_v1_compressed_01() {
        let bam_from_decompressed = {
            let data =
                DataSource::new(get_assets_path().join("BAM/V1/01/1chan03B_decompressed.BAM"));

            let mut reader = data.reader().unwrap();
            BamV1Parser::import(&mut reader).unwrap()
        };

        let bam_from_compressed = {
            let data = DataSource::new(get_assets_path().join("BAM/V1/01/1chan03B_compressed.BAM"));

            let mut reader = data.reader().unwrap();
            BamcParser::import(&mut reader).unwrap()
        };

        // BAMC and the corresponding plain BAM V1 differ only in the
        // wrapper tag (`Type::BamC` vs `Type::BamV1`); their decoded
        // content must match field-for-field.
        let Bam::V1(bam_from_compressed) = bam_from_compressed else {
            panic!("expected Bam::V1 from BAMC parser")
        };
        assert_eq!(bam_from_compressed.r#type, Type::BamC);
        assert_eq!(bam_from_decompressed.r#type, Type::BamV1);
        assert_eq!(bam_from_compressed.frames, bam_from_decompressed.frames);
        assert_eq!(bam_from_compressed.cycles, bam_from_decompressed.cycles);
        assert_eq!(bam_from_compressed.palette, bam_from_decompressed.palette);
        assert_eq!(
            bam_from_compressed.rle_compressed_color_index,
            bam_from_decompressed.rle_compressed_color_index
        );
    }

    #[test]
    fn test_parse_bam_v1_compressed_02() {
        let bam_from_decompressed = {
            let data =
                DataSource::new(get_assets_path().join("BAM/V1/02/SPHEART_decompressed.BAM"));

            let mut reader = data.reader().unwrap();
            BamV1Parser::import(&mut reader).unwrap()
        };

        let bam_from_compressed = {
            let data = DataSource::new(get_assets_path().join("BAM/V1/02/SPHEART_compressed.BAM"));

            let mut reader = data.reader().unwrap();
            BamcParser::import(&mut reader).unwrap()
        };

        // BAMC and the corresponding plain BAM V1 differ only in the
        // wrapper tag (`Type::BamC` vs `Type::BamV1`); their decoded
        // content must match field-for-field.
        let Bam::V1(bam_from_compressed) = bam_from_compressed else {
            panic!("expected Bam::V1 from BAMC parser")
        };
        assert_eq!(bam_from_compressed.r#type, Type::BamC);
        assert_eq!(bam_from_decompressed.r#type, Type::BamV1);
        assert_eq!(bam_from_compressed.frames, bam_from_decompressed.frames);
        assert_eq!(bam_from_compressed.cycles, bam_from_decompressed.cycles);
        assert_eq!(bam_from_compressed.palette, bam_from_decompressed.palette);
        assert_eq!(
            bam_from_compressed.rle_compressed_color_index,
            bam_from_decompressed.rle_compressed_color_index
        );
    }

    #[test]
    fn test_parse_bam_v1_compressed_03() {
        let bam_from_decompressed = {
            let data =
                DataSource::new(get_assets_path().join("BAM/V1/03/SPWI524D_decompressed.BAM"));

            let mut reader = data.reader().unwrap();
            BamV1Parser::import(&mut reader).unwrap()
        };

        let bam_from_compressed = {
            let data = DataSource::new(get_assets_path().join("BAM/V1/03/SPWI524D_compressed.BAM"));

            let mut reader = data.reader().unwrap();
            BamcParser::import(&mut reader).unwrap()
        };

        // BAMC and the corresponding plain BAM V1 differ only in the
        // wrapper tag (`Type::BamC` vs `Type::BamV1`); their decoded
        // content must match field-for-field.
        let Bam::V1(bam_from_compressed) = bam_from_compressed else {
            panic!("expected Bam::V1 from BAMC parser")
        };
        assert_eq!(bam_from_compressed.r#type, Type::BamC);
        assert_eq!(bam_from_decompressed.r#type, Type::BamV1);
        assert_eq!(bam_from_compressed.frames, bam_from_decompressed.frames);
        assert_eq!(bam_from_compressed.cycles, bam_from_decompressed.cycles);
        assert_eq!(bam_from_compressed.palette, bam_from_decompressed.palette);
        assert_eq!(
            bam_from_compressed.rle_compressed_color_index,
            bam_from_decompressed.rle_compressed_color_index
        );
    }
}
