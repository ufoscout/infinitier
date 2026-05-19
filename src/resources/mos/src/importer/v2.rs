//! MOS V2 parser.
//!
//! Wire layout (mirrors NearInfinity's `MosV2Decoder` and matches BAM V2's
//! data-block format byte-for-byte):
//!
//! ```text
//! 0    "MOS V2  "       (8 bytes)
//! 8    width            u32
//! 12   height           u32
//! 16   block_count      u32
//! 20   data_offset      u32  (always 24 — start of the block table)
//! 24   <block entries>  (block_count × 28 bytes)
//! ```
//!
//! Each block entry references a rectangle of pixels inside a `MOSnnnn.PVRZ`
//! page and a destination rectangle inside the MOS image. The importer
//! collects the metadata only — actual pixel decoding requires opening
//! the referenced PVRZ files (use the `pvrz` resource for that).

use std::io::{BufRead, Read, Seek};

use infinitier_datasource::{ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{MOS_V2_SIGNATURE, Type};

#[derive(Debug, PartialEq, Eq)]
pub struct MosV2 {
    pub r#type: Type,
    pub width: u32,
    pub height: u32,
    /// Data blocks in file order. Together they tile the entire
    /// `width × height` image (with no overlap in well-formed files).
    pub data_blocks: Vec<MosV2DataBlock>,
}

/// One MOS V2 data block — a rectangle copied from a PVRZ page into the
/// MOS image. Field layout matches `BamV2DataBlock` so consumers that
/// already handle BAM V2 atlases need no extra branching.
#[derive(Debug, PartialEq, Eq)]
pub struct MosV2DataBlock {
    /// PVRZ page index — resolves to the file `MOS<page:04d>.PVRZ`.
    pub pvrz_page: u32,
    pub width: u32,
    pub height: u32,
    pub source_x_coordinate: u32,
    pub source_y_coordinate: u32,
    pub target_x_coordinate: u32,
    pub target_y_coordinate: u32,
}

impl MosV2DataBlock {
    /// Returns the PVRZ filename this block sources from.
    pub fn pvrz_name(&self) -> String {
        format!("MOS{:04}.PVRZ", self.pvrz_page)
    }
}

/// A MOS V2 file importer.
pub struct MosV2Parser;

impl MosV2Parser {
    /// Parses a MOS V2 archive from a reader positioned at the start of
    /// the file (i.e. at the `"MOS V2  "` signature).
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<MosV2> {
        let mut signature = [0u8; 8];
        reader.read_exact(&mut signature)?;
        if &signature != MOS_V2_SIGNATURE {
            error!("Not a MOS V2 file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {:?}",
                signature
            )));
        }

        let width = reader.read_u32()?;
        let height = reader.read_u32()?;
        let block_count = reader.read_u32()? as usize;
        let data_offset = reader.read_u32()? as u64;

        if width == 0 || height == 0 {
            return Err(std::io::Error::other(format!(
                "Invalid MOS V2 dimensions: {width}x{height}"
            )));
        }

        reader.set_position(data_offset)?;
        let mut data_blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            data_blocks.push(MosV2DataBlock {
                pvrz_page: reader.read_u32()?,
                source_x_coordinate: reader.read_u32()?,
                source_y_coordinate: reader.read_u32()?,
                width: reader.read_u32()?,
                height: reader.read_u32()?,
                target_x_coordinate: reader.read_u32()?,
                target_y_coordinate: reader.read_u32()?,
            });
        }

        debug!(
            "Loaded MOS V2: {}x{}, {} data blocks",
            width,
            height,
            data_blocks.len()
        );
        Ok(MosV2 {
            r#type: Type::MosV2,
            width,
            height,
            data_blocks,
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::get_assets_path;

    use super::*;

    #[test]
    fn test_parse_mos_v2_should_fail_if_wrong_signature() {
        let data = DataSource::new(get_assets_path().join("MOS/V1/GTRSPCAP.mos"));
        let mut reader = data.reader().unwrap();
        let res = MosV2Parser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_mos_v2_bgdecbar() {
        // BGDECBAR.mos (BG2EE): an 864×58 horizontal decorative bar, tiled
        // from a single PVRZ page (MOS0015.PVRZ) using two blocks that
        // sit side-by-side: 0..512 and 512..864 on the x axis.
        let data = DataSource::new(get_assets_path().join("MOS/V2/BGDECBAR.mos"));
        let mut reader = data.reader().unwrap();
        let mos = MosV2Parser::import(&mut reader).unwrap();

        assert_eq!(mos.r#type, Type::MosV2);
        assert_eq!(mos.width, 864);
        assert_eq!(mos.height, 58);
        assert_eq!(mos.data_blocks.len(), 2);

        // Block 0: copies (0, 173)..(512, 231) of MOS0015.PVRZ into
        // (0, 0)..(512, 58) of the MOS image.
        let b0 = &mos.data_blocks[0];
        assert_eq!(b0.pvrz_page, 15);
        assert_eq!(b0.pvrz_name(), "MOS0015.PVRZ");
        assert_eq!(b0.source_x_coordinate, 0);
        assert_eq!(b0.source_y_coordinate, 173);
        assert_eq!(b0.width, 512);
        assert_eq!(b0.height, 58);
        assert_eq!(b0.target_x_coordinate, 0);
        assert_eq!(b0.target_y_coordinate, 0);

        // Block 1: covers the remaining 352 pixels of width, sourced from
        // a different rectangle of the same PVRZ.
        let b1 = &mos.data_blocks[1];
        assert_eq!(b1.pvrz_page, 15);
        assert_eq!(b1.source_x_coordinate, 1);
        assert_eq!(b1.source_y_coordinate, 233);
        assert_eq!(b1.width, 352);
        assert_eq!(b1.height, 58);
        assert_eq!(b1.target_x_coordinate, 512);
        assert_eq!(b1.target_y_coordinate, 0);

        // Sanity: blocks tile the full image area exactly.
        let total_area: u32 = mos.data_blocks.iter().map(|b| b.width * b.height).sum();
        assert_eq!(total_area, mos.width * mos.height);
    }

    #[test]
    fn test_pvrz_name_zero_padding() {
        let block = MosV2DataBlock {
            pvrz_page: 7,
            source_x_coordinate: 0,
            source_y_coordinate: 0,
            width: 0,
            height: 0,
            target_x_coordinate: 0,
            target_y_coordinate: 0,
        };
        assert_eq!(block.pvrz_name(), "MOS0007.PVRZ");

        let block = MosV2DataBlock {
            pvrz_page: 1234,
            ..block
        };
        assert_eq!(block.pvrz_name(), "MOS1234.PVRZ");
    }
}
