//! MOS V2 writer.
//!
//! Wire layout produced by this writer:
//!
//! ```text
//! 0    "MOS V2  "          (8 bytes)
//! 8    width               u32
//! 12   height              u32
//! 16   block_count         u32
//! 20   data_offset         u32  (always 24)
//! 24   <block entries>     (block_count × 28 bytes)
//! ```
//!
//! Like BAM V2, MOS V2 carries no inline pixel data — each entry points
//! at a rectangle inside a `MOS<page:04d>.PVRZ` page. The exporter only
//! writes the metadata; the referenced PVRZ files are untouched.

use crate::MOS_V2_SIGNATURE;
use crate::importer::v2::MosV2;

pub(super) fn build_mos_v2(mos: &MosV2) -> Vec<u8> {
    let block_count = mos.data_blocks.len() as u32;
    let data_offset: u32 = 24;
    let total = (data_offset + block_count * 28) as usize;

    let mut out = Vec::with_capacity(total);

    // 1. Header (24 bytes).
    out.extend_from_slice(MOS_V2_SIGNATURE);
    out.extend_from_slice(&mos.width.to_le_bytes());
    out.extend_from_slice(&mos.height.to_le_bytes());
    out.extend_from_slice(&block_count.to_le_bytes());
    out.extend_from_slice(&data_offset.to_le_bytes());

    // 2. Data block entries (28 bytes each). Field order matches the
    //    importer: pvrz_page, source_x, source_y, width, height,
    //    target_x, target_y — byte-for-byte the BAM V2 layout.
    for block in &mos.data_blocks {
        out.extend_from_slice(&block.pvrz_page.to_le_bytes());
        out.extend_from_slice(&block.source_x_coordinate.to_le_bytes());
        out.extend_from_slice(&block.source_y_coordinate.to_le_bytes());
        out.extend_from_slice(&block.width.to_le_bytes());
        out.extend_from_slice(&block.height.to_le_bytes());
        out.extend_from_slice(&block.target_x_coordinate.to_le_bytes());
        out.extend_from_slice(&block.target_y_coordinate.to_le_bytes());
    }

    debug_assert_eq!(out.len(), total);
    out
}
