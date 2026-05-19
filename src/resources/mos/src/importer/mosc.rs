//! MOSC parser — a zlib envelope around a complete MOS V1 archive.
//!
//! Wire layout:
//!
//! ```text
//! 0   "MOSCV1  "         (8 bytes)
//! 8   uncompressed_size  u32  (informational; the decoded MOS V1 must match)
//! 12  <zlib stream>      (decompresses to a "MOS V1  " archive)
//! ```
//!
//! The decompressed payload is parsed via the regular MOS V1 importer;
//! the only difference visible in the returned [`Mos`] is that the
//! `r#type` tag is set to [`Type::Mosc`] so the wrapper choice is
//! preserved for future round-tripping. Same pattern as BAMC.

use std::io::{BufRead, Read};

use infinitier_datasource::{ReadExt, Reader};
use log::{debug, error};

use crate::{Mos, MOSC_V1_SIGNATURE, Type};

use super::MosImporter;

/// A MOSC file importer.
pub struct MoscParser;

impl MoscParser {
    /// Parses a MOSC archive from a reader positioned at the start of the
    /// file (i.e. at the `"MOSCV1  "` signature).
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Mos> {
        let mut signature = [0u8; 8];
        reader.read_exact(&mut signature)?;
        if &signature != MOSC_V1_SIGNATURE {
            error!("Not a MOSC file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {:?}",
                signature
            )));
        }

        // `uncompressed_size` is informational — the inner MOS V1 header
        // carries the same information through its width/height/cols/rows
        // fields, and the zlib stream tells us when to stop. NearInfinity
        // reads but does not validate this field, and so do we.
        let _uncompressed_size = reader.read_u32()?;

        debug!("Decompressing MOSC data");
        let mut uncompressed_reader = reader.as_zip_reader().decode_all()?;

        let inner = MosImporter::from_reader(&mut uncompressed_reader)?;
        Ok(match inner {
            Mos::V1(mut v1) => {
                v1.r#type = Type::Mosc;
                Mos::V1(v1)
            }
            // MOSC in practice only wraps MOS V1 — leave any V2 payload
            // alone if it ever appears.
            Mos::V2(v2) => Mos::V2(v2),
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    use super::*;
    use super::super::v1::MosV1Parser;

    #[test]
    fn test_parse_mosc_should_fail_if_wrong_signature() {
        let data = DataSource::new(get_assets_path().join("MOS/V1/GTRSPCAP.mos"));
        let mut reader = data.reader().unwrap();
        let res = MoscParser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_mosc_guiwrlp8() {
        // GUIWRLP8.mos (IWD): MOSC wrapper around a 6×600 MOS V1 image
        // (1 column × 10 rows).
        let data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let mut reader = data.reader().unwrap();
        let mos = MoscParser::import(&mut reader).unwrap();

        let Mos::V1(v1) = mos else {
            panic!("expected Mos::V1");
        };
        assert_eq!(v1.r#type, Type::Mosc);
        assert_eq!(v1.width, 6);
        assert_eq!(v1.height, 600);
        assert_eq!(v1.columns, 1);
        assert_eq!(v1.rows, 10);
        assert_eq!(v1.blocks.len(), 10);

        // Rows 0..9 are full-height 6×64 blocks; the last row truncates.
        for (i, block) in v1.blocks.iter().enumerate() {
            assert_eq!(block.width, 6, "block {i} width");
            let expected_height = if i + 1 < 10 { 64 } else { 600 - 9 * 64 };
            assert_eq!(block.height, expected_height, "block {i} height");
            assert_eq!(
                block.pixel_palette_indexes.len() as u32,
                block.width * block.height,
                "block {i} pixel count"
            );
            assert_eq!(block.palette.len(), 256);
        }
    }

    #[test]
    fn test_parse_mosc_matches_decompressed_v1() {
        // The decoded content of a MOSC archive must match the plain MOS
        // V1 file you'd get by decompressing it externally; the only
        // expected difference is the `r#type` tag (Mosc vs MosV1).
        let mosc_data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let mosc = MoscParser::import(&mut mosc_data.reader().unwrap()).unwrap();
        let Mos::V1(mosc) = mosc else {
            panic!("expected Mos::V1");
        };

        let dec_data =
            DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8_decompressed.mos"));
        let dec = MosV1Parser::import(&mut dec_data.reader().unwrap()).unwrap();

        assert_eq!(mosc.r#type, Type::Mosc);
        assert_eq!(dec.r#type, Type::MosV1);
        assert_eq!(mosc.width, dec.width);
        assert_eq!(mosc.height, dec.height);
        assert_eq!(mosc.columns, dec.columns);
        assert_eq!(mosc.rows, dec.rows);
        assert_eq!(mosc.blocks, dec.blocks);
    }

    #[test]
    fn test_mosc_decompressed_bytes_match_sidecar() {
        // The zlib payload inside GUIWRLP8.mos must inflate byte-for-byte
        // to the GUIWRLP8_decompressed.mos sidecar — a stronger check than
        // the parsed-equivalence test, since it catches issues that the
        // parser would silently normalize away.
        let data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let mut reader = data.reader().unwrap();
        let mut signature = [0u8; 8];
        reader.read_exact(&mut signature).unwrap();
        assert_eq!(&signature, MOSC_V1_SIGNATURE);
        let _uncompressed_size = reader.read_u32().unwrap();

        let mut decompressed = Vec::new();
        reader
            .as_zip_reader()
            .decode_all()
            .unwrap()
            .read_to_end(&mut decompressed)
            .unwrap();

        let expected =
            std::fs::read(get_assets_path().join("MOS/MOSC/GUIWRLP8_decompressed.mos")).unwrap();
        assert_eq!(decompressed, expected);
    }

    #[test]
    fn test_mosc_guiwrlp8_matches_png() {
        let data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let mos = MoscParser::import(&mut data.reader().unwrap()).unwrap();
        let Mos::V1(v1) = mos else {
            panic!("expected Mos::V1");
        };

        assert_images_are_equal(
            &image::open(get_assets_path().join("MOS/MOSC/GUIWRLP8.png")).unwrap(),
            &v1.to_image().into(),
            None,
        );
    }
}
