//! TIS palette writer + image-to-TIS converter.
//!
//! Wire layout produced by this writer (matches the importer):
//!
//! ```text
//! 0    "TIS V1  "       (8 bytes)
//! 8    tile_count       u32
//! 12   tile_length      u32  (5120 — 1024 palette + 4096 indices)
//! 16   header_size      u32  (24)
//! 20   tile_dimension   u32  (64)
//! 24   <tiles>          (tile_count × 5120 bytes)
//! ```
//!
//! Tile bytes are emitted exactly as the importer reads them — 256 BGRA
//! palette entries followed by 4096 row-major pixel indices — so plain
//! round-trip (`import → export`) is byte-for-byte identical.

use std::collections::HashMap;
use std::io;

use color_quant::NeuQuant;
use image::{ImageBuffer, Rgba};

use crate::common::Rgb;
use crate::importer::palette::{TisPalette, TisPaletteTile};
use crate::{PALETTE_TILE_LENGTH, TILE_DIMENSION, TIS_SIGNATURE, Type};

pub(super) fn build_tis_palette(tis: &TisPalette) -> Vec<u8> {
    let tile_count = tis.tiles.len() as u32;
    let header_size: u32 = 24;
    let total = (header_size + tile_count * PALETTE_TILE_LENGTH) as usize;
    let mut out = Vec::with_capacity(total);

    // 1. Header (24 bytes).
    out.extend_from_slice(TIS_SIGNATURE);
    out.extend_from_slice(&tile_count.to_le_bytes());
    out.extend_from_slice(&PALETTE_TILE_LENGTH.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&tis.tile_dimension.to_le_bytes());

    // 2. Tiles — palette (BGRA, 1024 bytes) then 4096 pixel indices.
    for tile in &tis.tiles {
        for entry in &tile.palette {
            out.push(entry.b);
            out.push(entry.g);
            out.push(entry.r);
            out.push(entry.alpha);
        }
        // Zero-pad the palette if a caller built a tile with fewer than
        // 256 entries; the importer always reads exactly 1024 bytes.
        let pad = 256usize.saturating_sub(tile.palette.len()) * 4;
        out.extend(std::iter::repeat_n(0u8, pad));

        out.extend_from_slice(&tile.pixel_palette_indexes);
    }

    debug_assert_eq!(out.len(), total);
    out
}

/// Converts an arbitrary RGBA image into a [`TisPalette`] by splitting
/// it into 64×64 tiles (row-major) and quantizing each tile to a
/// 256-entry palette. See [`super::TisExporter::image_to_tis_palette`].
pub fn image_to_tis_palette(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> io::Result<TisPalette> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err(io::Error::other(
            "image must have non-zero dimensions to encode as TIS",
        ));
    }
    if !width.is_multiple_of(TILE_DIMENSION) || !height.is_multiple_of(TILE_DIMENSION) {
        return Err(io::Error::other(format!(
            "image dimensions must be multiples of 64 to encode as TIS (got {width}x{height})"
        )));
    }
    let columns = width / TILE_DIMENSION;
    let rows = height / TILE_DIMENSION;
    let tile_count = (columns as usize) * (rows as usize);

    let mut tiles: Vec<TisPaletteTile> = Vec::with_capacity(tile_count);
    for row in 0..rows {
        for col in 0..columns {
            let x0 = col * TILE_DIMENSION;
            let y0 = row * TILE_DIMENSION;
            tiles.push(build_tile(image, x0, y0));
        }
    }

    Ok(TisPalette {
        r#type: Type::Palette,
        tile_dimension: TILE_DIMENSION,
        tiles,
    })
}

/// Per-tile palette + indices builder.
///
/// Pixels with `alpha == 0` are routed to palette index 0 holding the
/// magic green `RGB(0, 255, 0)` — the only entry NearInfinity's TIS V1
/// renderer treats as transparent. If a tile has no transparent pixel
/// the magic-green slot is dropped and all 256 slots hold real colors;
/// otherwise opaque colors fit in the remaining 255 slots.
///
/// When the unique opaque color count already fits in the available
/// slots the palette is built directly (no quantization loss).
/// Otherwise NeuQuant produces a representative palette and `index_of`
/// selects each pixel. Mirrors the corresponding helper in
/// `infinitier_mos_resource`'s exporter.
fn build_tile(image: &ImageBuffer<Rgba<u8>, Vec<u8>>, x0: u32, y0: u32) -> TisPaletteTile {
    let pixel_count = (TILE_DIMENSION * TILE_DIMENSION) as usize;

    let mut has_transparent = false;
    let mut opaque_pixels: Vec<[u8; 4]> = Vec::with_capacity(pixel_count);
    let mut opaque_positions: Vec<usize> = Vec::with_capacity(pixel_count);
    let mut transparency_mask: Vec<bool> = vec![false; pixel_count];

    for y in 0..TILE_DIMENSION {
        for x in 0..TILE_DIMENSION {
            let i = (y * TILE_DIMENSION + x) as usize;
            let p = image.get_pixel(x0 + x, y0 + y).0;
            if p[3] == 0 {
                has_transparent = true;
                transparency_mask[i] = true;
            } else {
                // Force alpha to 255 — TIS palette alpha is unused at
                // render time, so we don't want NeuQuant clustering by
                // an axis the engine discards.
                opaque_pixels.push([p[0], p[1], p[2], 255]);
                opaque_positions.push(i);
            }
        }
    }

    let reserve_transparency = has_transparent;
    let available_slots = if reserve_transparency { 255 } else { 256 };
    let opaque_start_index: u8 = if reserve_transparency { 1 } else { 0 };

    let mut palette: Vec<Rgb> = Vec::with_capacity(256);
    let mut pixel_palette_indexes = vec![0u8; pixel_count];

    if reserve_transparency {
        // Index 0 is the magic-green transparency slot. The engine
        // ignores the alpha byte; vanilla files store 0x00 here.
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

    // Try the no-loss path first: collect unique opaque RGB triples and
    // see whether they fit in the remaining palette slots.
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
        // Push in stable insertion order so indices line up with
        // what's already in `unique_colors`.
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
        // NeuQuant requires `colors >= 64`; we only reach this branch
        // when the unique color count is already >255, so the request
        // (255 or 256) is always large enough. samplefac=10 is the
        // documented speed/quality midpoint.
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

    // Pad the palette to exactly 256 entries — the importer always
    // reads 1024 bytes here, no matter how many we actually used.
    while palette.len() < 256 {
        palette.push(Rgb {
            r: 0,
            g: 0,
            b: 0,
            alpha: 0,
        });
    }

    TisPaletteTile {
        palette,
        pixel_palette_indexes,
    }
}
