//! MOS V1 writer.
//!
//! Wire layout produced by this writer:
//!
//! ```text
//! 0    "MOS V1  "         (8 bytes)
//! 8    width              u16
//! 10   height             u16
//! 12   columns            u16
//! 14   rows               u16
//! 16   block_size         u32  (always 64)
//! 20   palette_offset     u32  (always 24)
//! 24   <palettes>         (columns*rows × 1024 bytes — one 256-entry BGRA palette per block)
//!      <lookup table>     (columns*rows × u32 — per-block data offset relative to the data section)
//!      <pixel data>       (block pixels back-to-back, no padding)
//! ```
//!
//! Pixel data is packed back-to-back in block order; the lookup table is
//! filled with the cumulative byte offsets that result. Re-import yields
//! the same `MosV1` content (palettes and `pixel_palette_indexes`) the
//! exporter started from.

use std::collections::HashMap;

use color_quant::NeuQuant;
use image::{ImageBuffer, Rgba};

use crate::common::Rgb;
use crate::importer::v1::{BLOCK_DIMENSION, MosV1, MosV1Block};
use crate::{MOS_V1_SIGNATURE, Type};

pub(super) fn build_mos_v1(mos: &MosV1) -> Vec<u8> {
    let block_count = mos.blocks.len();
    let palette_offset: u32 = 24;
    let palette_size = (block_count as u32) * 1024;
    let lookup_offset = palette_offset + palette_size;
    let lookup_size = (block_count as u32) * 4;
    let data_section_start = lookup_offset + lookup_size;

    // Per-block byte offsets into the data section, computed sequentially.
    let mut block_data_offsets: Vec<u32> = Vec::with_capacity(block_count);
    let mut cursor: u32 = 0;
    for block in &mos.blocks {
        block_data_offsets.push(cursor);
        cursor += block.pixel_palette_indexes.len() as u32;
    }
    let total = (data_section_start + cursor) as usize;
    let mut out = Vec::with_capacity(total);

    // 1. Header (24 bytes).
    out.extend_from_slice(MOS_V1_SIGNATURE);
    out.extend_from_slice(&(mos.width as u16).to_le_bytes());
    out.extend_from_slice(&(mos.height as u16).to_le_bytes());
    out.extend_from_slice(&mos.columns.to_le_bytes());
    out.extend_from_slice(&mos.rows.to_le_bytes());
    out.extend_from_slice(&BLOCK_DIMENSION.to_le_bytes());
    out.extend_from_slice(&palette_offset.to_le_bytes());

    // 2. Palettes (256 BGRA entries per block, in block order). Padding
    //    past `block.palette.len()` is zero-filled — the importer always
    //    reads exactly 256 entries.
    for block in &mos.blocks {
        for entry in &block.palette {
            out.push(entry.b);
            out.push(entry.g);
            out.push(entry.r);
            out.push(entry.alpha);
        }
        let pad = 256usize.saturating_sub(block.palette.len()) * 4;
        out.extend(std::iter::repeat_n(0u8, pad));
    }

    // 3. Lookup table (one u32 offset per block, into the data section).
    for &offset in &block_data_offsets {
        out.extend_from_slice(&offset.to_le_bytes());
    }

    // 4. Per-block pixel data, packed back-to-back.
    for block in &mos.blocks {
        out.extend_from_slice(&block.pixel_palette_indexes);
    }

    debug_assert_eq!(out.len(), total);
    out
}

/// Converts an arbitrary RGBA image into a [`MosV1`] by splitting it into
/// 64×64 blocks and quantizing each block independently to a 256-entry
/// palette. See [`super::MosExporter::image_to_mos_v1`].
pub fn image_to_mos_v1(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> MosV1 {
    let width = image.width();
    let height = image.height();
    let columns = width.div_ceil(BLOCK_DIMENSION) as u16;
    let rows = height.div_ceil(BLOCK_DIMENSION) as u16;
    let block_count = columns as usize * rows as usize;

    let mut blocks: Vec<MosV1Block> = Vec::with_capacity(block_count);
    for row in 0..rows as u32 {
        for col in 0..columns as u32 {
            let x0 = col * BLOCK_DIMENSION;
            let y0 = row * BLOCK_DIMENSION;
            let block_width = (width - x0).min(BLOCK_DIMENSION);
            let block_height = (height - y0).min(BLOCK_DIMENSION);

            blocks.push(build_block(image, x0, y0, block_width, block_height));
        }
    }

    MosV1 {
        r#type: Type::MosV1,
        width,
        height,
        columns,
        rows,
        blocks,
    }
}

/// Per-block palette + indices builder.
///
/// Pixels with `alpha == 0` are routed to the MOS V1 "magic green"
/// transparency entry — `RGB(0, 255, 0)`, file alpha `0x00`. If the
/// block has any transparent pixel, one palette slot is reserved for
/// that entry and the opaque pixels are quantized into the remaining
/// 255 slots; otherwise all 256 slots are used for opaque colors.
///
/// When the unique opaque color count already fits, the palette is
/// built directly (no quantization loss). Otherwise NeuQuant produces
/// a representative palette and `index_of` selects each pixel.
fn build_block(
    image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    x0: u32,
    y0: u32,
    block_width: u32,
    block_height: u32,
) -> MosV1Block {
    let pixel_count = (block_width as usize) * (block_height as usize);

    let mut has_transparent = false;
    let mut opaque_pixels: Vec<[u8; 4]> = Vec::with_capacity(pixel_count);
    let mut opaque_positions: Vec<usize> = Vec::with_capacity(pixel_count);
    let mut transparency_mask: Vec<bool> = vec![false; pixel_count];

    for y in 0..block_height {
        for x in 0..block_width {
            let i = (y as usize) * (block_width as usize) + (x as usize);
            let p = image.get_pixel(x0 + x, y0 + y).0;
            if p[3] == 0 {
                has_transparent = true;
                transparency_mask[i] = true;
            } else {
                // Force alpha to 255 so NeuQuant doesn't weight pixels by
                // an axis we discard on disk anyway.
                opaque_pixels.push([p[0], p[1], p[2], 255]);
                opaque_positions.push(i);
            }
        }
    }

    // Magic-green slot at index 0 when we need to encode transparency.
    let reserve_transparency = has_transparent;
    let available_slots = if reserve_transparency { 255 } else { 256 };
    let opaque_start_index: u8 = if reserve_transparency { 1 } else { 0 };

    let mut palette: Vec<Rgb> = Vec::with_capacity(256);
    let mut pixel_palette_indexes = vec![0u8; pixel_count];

    if reserve_transparency {
        // Vanilla convention: the transparency entry is stored as
        // RGB(0, 255, 0) with file alpha = 0x00. The on-disk alpha is
        // ignored by the renderer; it picks transparency from RGB.
        palette.push(Rgb {
            r: 0,
            g: 255,
            b: 0,
            alpha: 0,
        });
        for (i, is_t) in transparency_mask.iter().enumerate() {
            if *is_t {
                pixel_palette_indexes[i] = 0;
            }
        }
    }

    // Determine unique opaque colors (RGB only).
    let mut unique_colors: HashMap<(u8, u8, u8), u8> = HashMap::new();
    let mut direct_palette_fits = true;
    for px in &opaque_pixels {
        let key = (px[0], px[1], px[2]);
        if !unique_colors.contains_key(&key) {
            if unique_colors.len() >= available_slots {
                direct_palette_fits = false;
                break;
            }
            let new_index = (unique_colors.len() as u8) + opaque_start_index;
            unique_colors.insert(key, new_index);
        }
    }

    if direct_palette_fits {
        // Push entries in stable insertion order so the assigned indices
        // line up with what's already in `unique_colors`.
        let mut ordered: Vec<((u8, u8, u8), u8)> =
            unique_colors.iter().map(|(k, v)| (*k, *v)).collect();
        ordered.sort_by_key(|(_, idx)| *idx);
        for ((r, g, b), _) in ordered {
            palette.push(Rgb { r, g, b, alpha: 0 });
        }
        for (px, &pos) in opaque_pixels.iter().zip(opaque_positions.iter()) {
            let key = (px[0], px[1], px[2]);
            pixel_palette_indexes[pos] = unique_colors[&key];
        }
    } else {
        // NeuQuant requires `colors >= 64`; we only land here when the
        // unique color count already exceeded 255, so `available_slots`
        // (≥255) is always large enough. samplefac=10 is the documented
        // speed/quality midpoint.
        let mut flat: Vec<u8> = Vec::with_capacity(opaque_pixels.len() * 4);
        for px in &opaque_pixels {
            flat.extend_from_slice(px);
        }
        let nq = NeuQuant::new(10, available_slots, &flat);
        let rgba_map = nq.color_map_rgba();
        for entry in rgba_map.chunks_exact(4) {
            palette.push(Rgb {
                r: entry[0],
                g: entry[1],
                b: entry[2],
                alpha: 0,
            });
        }
        for (px, &pos) in opaque_pixels.iter().zip(opaque_positions.iter()) {
            let nq_index = nq.index_of(px) as u8;
            pixel_palette_indexes[pos] = nq_index + opaque_start_index;
        }
    }

    // Pad to a full 256-entry palette regardless of how many slots we
    // actually used; the importer always reads 1024 bytes per block.
    while palette.len() < 256 {
        palette.push(Rgb {
            r: 0,
            g: 0,
            b: 0,
            alpha: 0,
        });
    }

    MosV1Block {
        width: block_width,
        height: block_height,
        palette,
        pixel_palette_indexes,
    }
}
