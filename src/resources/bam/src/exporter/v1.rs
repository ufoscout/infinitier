//! BAM V1 writer.
//!
//! Wire layout produced by this writer:
//!
//! ```text
//! 0    "BAM V1  "          (8 bytes)
//! 8    frames_count        u16
//! 10   cycles_count        u8
//! 11   rle_color_index     u8
//! 12   frames_offset       u32   (always 0x18)
//! 16   palette_offset      u32
//! 20   lookup_offset       u32
//! 24   <frame entries>     (frames_count * 12 bytes)
//!      <cycle entries>     (cycles_count * 4 bytes)        — immediately follows frames
//!      <palette>           (palette.len() * 4 bytes)
//!      <lookup table>      (sum of cycle frame_indices * 2 bytes)
//!      <pixel data>        (per-frame, uncompressed)
//! ```
//!
//! Pixel data is always emitted uncompressed (`data_bits` top bit = 1).
//! Re-import yields the same `pixel_palette_indexes` regardless of whether
//! the source frame was originally RLE-compressed — the user accepted
//! "functionally identical, not byte-exact" as the round-trip criterion.

use crate::BAM_V1_SIGNATURE;
use crate::importer::v1::BamV1;

pub(super) fn build_bam_v1(bam: &BamV1) -> Vec<u8> {
    // Compute section sizes up front so we can write a single forward pass.
    let frames_table_size = (bam.frames.len() as u32) * 12;
    let cycles_table_size = (bam.cycles.len() as u32) * 4;
    let palette_size = (bam.palette.len() as u32) * 4;
    let lookup_size = bam
        .cycles
        .iter()
        .map(|c| c.frame_indices.len() as u32)
        .sum::<u32>()
        * 2;

    let frames_offset: u32 = 24; // header is 24 bytes
    let cycles_offset = frames_offset + frames_table_size; // unused in header but needed in flow
    let palette_offset = cycles_offset + cycles_table_size;
    let lookup_offset = palette_offset + palette_size;
    let first_frame_data_offset = lookup_offset + lookup_size;

    // Per-frame data offsets, computed sequentially.
    let mut frame_data_offsets = Vec::with_capacity(bam.frames.len());
    let mut cursor = first_frame_data_offset;
    for frame in &bam.frames {
        frame_data_offsets.push(cursor);
        cursor += frame.pixel_palette_indexes.len() as u32;
    }
    let total = cursor as usize;
    let mut out = Vec::with_capacity(total);

    // 1. Header (24 bytes).
    out.extend_from_slice(BAM_V1_SIGNATURE.as_bytes());
    out.extend_from_slice(&(bam.frames.len() as u16).to_le_bytes());
    out.push(bam.cycles.len() as u8);
    out.push(bam.rle_compressed_color_index);
    out.extend_from_slice(&frames_offset.to_le_bytes());
    out.extend_from_slice(&palette_offset.to_le_bytes());
    out.extend_from_slice(&lookup_offset.to_le_bytes());

    // 2. Frame entries (12 bytes each). data_bits top-bit = 1 marks
    //    "uncompressed"; the lower 31 bits hold the data offset.
    for (frame, &data_offset) in bam.frames.iter().zip(frame_data_offsets.iter()) {
        out.extend_from_slice(&(frame.width as u16).to_le_bytes());
        out.extend_from_slice(&(frame.height as u16).to_le_bytes());
        out.extend_from_slice(&(frame.center_x as i16).to_le_bytes());
        out.extend_from_slice(&(frame.center_y as i16).to_le_bytes());
        let data_bits = data_offset | 0x8000_0000;
        out.extend_from_slice(&data_bits.to_le_bytes());
    }

    // 3. Cycle entries (4 bytes each). `lookup_table_index` is the starting
    //    u16 index into the global lookup table; we pack cycles' indices
    //    back-to-back (no dedup).
    let mut lookup_cursor: u16 = 0;
    for cycle in &bam.cycles {
        let indices_count = cycle.frame_indices.len() as u16;
        out.extend_from_slice(&indices_count.to_le_bytes());
        out.extend_from_slice(&lookup_cursor.to_le_bytes());
        lookup_cursor = lookup_cursor.wrapping_add(indices_count);
    }

    // 4. Palette (BGRA per entry). For the transparency entry — RGB(0,255,0)
    //    with alpha=0 in memory — the importer maps file alpha=0 to memory
    //    alpha=255 *then* forces the transparency entry's alpha back to 0,
    //    so writing alpha=0 verbatim round-trips correctly.
    for entry in &bam.palette {
        out.push(entry.b);
        out.push(entry.g);
        out.push(entry.r);
        out.push(entry.alpha);
    }

    // 5. Lookup table (u16 frame indices, in cycle order).
    for cycle in &bam.cycles {
        for &idx in &cycle.frame_indices {
            out.extend_from_slice(&(idx as u16).to_le_bytes());
        }
    }

    // 6. Frame pixel data — raw, uncompressed.
    for frame in &bam.frames {
        out.extend_from_slice(&frame.pixel_palette_indexes);
    }

    debug_assert_eq!(out.len(), total);
    out
}
