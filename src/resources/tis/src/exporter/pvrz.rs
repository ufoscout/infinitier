//! TIS PVRZ writer.
//!
//! Wire layout produced by this writer (matches the importer):
//!
//! ```text
//! 0    "TIS V1  "       (8 bytes — shared with the palette variant)
//! 8    tile_count       u32
//! 12   tile_length      u32  (12 — discriminates from the palette variant)
//! 16   header_size      u32  (24)
//! 20   tile_dimension   u32  (64)
//! 24   <tile entries>   (tile_count × 12 bytes)
//! ```
//!
//! Each tile entry is `(pvrz_page: i32, source_x: u32, source_y: u32)`.
//! The exporter doesn't open or rewrite the referenced PVRZ pages —
//! the [`TisPvrz`] struct only carries metadata, and so does the output.

use crate::importer::pvrz::TisPvrz;
use crate::{PVRZ_TILE_LENGTH, TIS_SIGNATURE};

pub(super) fn build_tis_pvrz(tis: &TisPvrz) -> Vec<u8> {
    let tile_count = tis.tiles.len() as u32;
    let header_size: u32 = 24;
    let total = (header_size + tile_count * PVRZ_TILE_LENGTH) as usize;
    let mut out = Vec::with_capacity(total);

    // 1. Header (24 bytes).
    out.extend_from_slice(TIS_SIGNATURE);
    out.extend_from_slice(&tile_count.to_le_bytes());
    out.extend_from_slice(&PVRZ_TILE_LENGTH.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&tis.tile_dimension.to_le_bytes());

    // 2. Tile entries (12 bytes each). The "no source" sentinel
    //    (`pvrz_page == -1`) is written verbatim as `0xFFFFFFFF`.
    for tile in &tis.tiles {
        out.extend_from_slice(&tile.pvrz_page.to_le_bytes());
        out.extend_from_slice(&tile.source_x.to_le_bytes());
        out.extend_from_slice(&tile.source_y.to_le_bytes());
    }

    debug_assert_eq!(out.len(), total);
    out
}
