//! BAM V2 writer.
//!
//! Wire layout produced by this writer:
//!
//! ```text
//! 0    "BAM V2  "             (8 bytes)
//! 8    frames_count           u32
//! 12   cycles_count           u32
//! 16   data_blocks_count      u32
//! 20   frames_offset          u32   (always 0x20)
//! 24   cycles_offset          u32
//! 28   data_blocks_offset     u32
//! 32   <frame entries>        (frames_count * 12 bytes)
//!      <cycle entries>        (cycles_count * 4 bytes)
//!      <data block entries>   (data_blocks_count * 28 bytes)
//! ```
//!
//! Unlike BAM V1, V2 contains no inline pixel data — pixels live in
//! per-page PVRZ files referenced via `BamV2DataBlock::pvrz_page`. The
//! exporter does not touch those PVRZ files.

use crate::BAM_V2_SIGNATURE;
use crate::importer::v2::BamV2;

pub(super) fn build_bam_v2(bam: &BamV2) -> Vec<u8> {
    let frames_table_size = (bam.frames.len() as u32) * 12;
    let cycles_table_size = (bam.cycles.len() as u32) * 4;
    let data_blocks_table_size = (bam.data_blocks.len() as u32) * 28;

    let frames_offset: u32 = 32;
    let cycles_offset = frames_offset + frames_table_size;
    let data_blocks_offset = cycles_offset + cycles_table_size;
    let total = (data_blocks_offset + data_blocks_table_size) as usize;

    let mut out = Vec::with_capacity(total);

    // 1. Header (32 bytes).
    out.extend_from_slice(BAM_V2_SIGNATURE.as_bytes());
    out.extend_from_slice(&(bam.frames.len() as u32).to_le_bytes());
    out.extend_from_slice(&(bam.cycles.len() as u32).to_le_bytes());
    out.extend_from_slice(&(bam.data_blocks.len() as u32).to_le_bytes());
    out.extend_from_slice(&frames_offset.to_le_bytes());
    out.extend_from_slice(&cycles_offset.to_le_bytes());
    out.extend_from_slice(&data_blocks_offset.to_le_bytes());

    // 2. Frame entries (12 bytes each).
    for frame in &bam.frames {
        out.extend_from_slice(&(frame.width as u16).to_le_bytes());
        out.extend_from_slice(&(frame.height as u16).to_le_bytes());
        out.extend_from_slice(&(frame.center_x as i16).to_le_bytes());
        out.extend_from_slice(&(frame.center_y as i16).to_le_bytes());
        out.extend_from_slice(&(frame.data_blocks_start_index as u16).to_le_bytes());
        out.extend_from_slice(&(frame.data_blocks_count as u16).to_le_bytes());
    }

    // 3. Cycle entries (4 bytes each).
    for cycle in &bam.cycles {
        out.extend_from_slice(&(cycle.frames_count as u16).to_le_bytes());
        out.extend_from_slice(&(cycle.frame_start_index as u16).to_le_bytes());
    }

    // 4. Data block entries (28 bytes each).
    for block in &bam.data_blocks {
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
