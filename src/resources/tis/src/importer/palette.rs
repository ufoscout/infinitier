//! TIS palette parser.
//!
//! Wire layout (mirrors NearInfinity's `TisV1Decoder`):
//!
//! ```text
//! 0    "TIS V1  "       (8 bytes)
//! 8    tile_count       u32  (number of tiles)
//! 12   tile_length      u32  (always 5120 — 1024 palette + 4096 pixels)
//! 16   header_size      u32  (always 24 — offset to first tile)
//! 20   tile_dimension   u32  (always 64)
//! 24   <tiles>          (tile_count × 5120 bytes)
//! ```
//!
//! Each tile is 5120 bytes back-to-back:
//! - 1024 bytes — 256 BGRA palette entries (alpha unused)
//! - 4096 bytes — 64×64 paletted pixel indices, row-major
//!
//! Tiles have no spatial ordering of their own; the consuming `WED`
//! resource defines how they are laid out into the area map.

use std::io::{BufRead, Read, Seek};

use image::{ImageBuffer, Rgba};
use infinitier_datasource::{ReadExt, Reader};
use log::{debug, error};

use crate::common::Rgb;
use crate::{PALETTE_TILE_LENGTH, TILE_DIMENSION, TIS_SIGNATURE, Type};

/// One TIS palette tile — a self-contained 64×64 image with its own
/// 256-entry palette.
#[derive(Debug, PartialEq, Eq)]
pub struct TisPaletteTile {
    /// 256 BGRA entries from disk. The alpha byte is conventionally
    /// ignored by the engine; see [`TisPalette::to_image`] for the
    /// transparency rule.
    pub palette: Vec<Rgb>,
    /// `64 * 64 = 4096` palette indices, row-major from top-left.
    pub pixel_palette_indexes: Vec<u8>,
}

/// A TIS palette-variant archive (NearInfinity's `Type.PALETTE`).
#[derive(Debug, PartialEq, Eq)]
pub struct TisPalette {
    /// `Type::Palette` — present so callers can write
    /// `match tis { Tis::Palette(p) => p.r#type, … }` uniformly with
    /// the PVRZ variant.
    pub r#type: Type,
    /// Width / height of each tile in pixels (always 64 in well-formed
    /// files; the parser validates this).
    pub tile_dimension: u32,
    /// Tiles in file order. There is no implicit row/column layout —
    /// that lives in the `.wed` overlay map.
    pub tiles: Vec<TisPaletteTile>,
}

impl TisPalette {
    /// Renders a single tile into an RGBA buffer.
    ///
    /// Mirrors NearInfinity's `TisV1Decoder.renderTile`: palette index 0
    /// is rendered fully transparent only when its RGB is the magic
    /// green `(0, 255, 0)`; every other entry is forced opaque
    /// regardless of the file's stored alpha byte (which the engine
    /// ignores). Panics if `index >= self.tiles.len()`.
    pub fn tile_image(&self, index: usize) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let mut img = ImageBuffer::new(self.tile_dimension, self.tile_dimension);
        self.paint_tile(index, &mut img, 0, 0);
        img
    }

    /// Stitches every tile into a single RGBA image laid out row-major
    /// in a `columns × ceil(tile_count / columns)` grid.
    ///
    /// TIS files have no inherent row/column count — that information
    /// lives in the area's `.wed` overlay map — so the caller picks how
    /// many columns the grid should have. The last row may be padded
    /// with transparent pixels if `tile_count` is not a multiple of
    /// `columns`. Panics if `columns == 0`.
    pub fn to_image(&self, columns: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        assert!(columns > 0, "columns must be > 0");
        let rows = (self.tiles.len() as u32).div_ceil(columns);
        let mut img =
            ImageBuffer::new(columns * self.tile_dimension, rows * self.tile_dimension);
        for idx in 0..self.tiles.len() {
            let col = (idx as u32) % columns;
            let row = (idx as u32) / columns;
            let x0 = col * self.tile_dimension;
            let y0 = row * self.tile_dimension;
            self.paint_tile(idx, &mut img, x0, y0);
        }
        img
    }

    /// Internal: paints tile `index` into `dst` at offset `(x0, y0)`.
    /// Shared between [`Self::tile_image`] and [`Self::to_image`] so the
    /// transparency rule stays in one place.
    fn paint_tile(
        &self,
        index: usize,
        dst: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x0: u32,
        y0: u32,
    ) {
        let tile = &self.tiles[index];
        let transparent_zero =
            tile.palette[0].r == 0 && tile.palette[0].g == 255 && tile.palette[0].b == 0;
        for y in 0..self.tile_dimension {
            for x in 0..self.tile_dimension {
                let i = (y * self.tile_dimension + x) as usize;
                let palette_index = tile.pixel_palette_indexes[i] as usize;
                let p = &tile.palette[palette_index];
                let alpha = if palette_index == 0 && transparent_zero {
                    0
                } else {
                    255
                };
                dst.put_pixel(x0 + x, y0 + y, Rgba([p.r, p.g, p.b, alpha]));
            }
        }
    }
}

/// A TIS palette-variant file importer.
pub struct TisPaletteParser;

impl TisPaletteParser {
    /// Parses a TIS palette archive from a reader positioned at the
    /// start of the file (i.e. at the `"TIS V1  "` signature).
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<TisPalette> {
        let mut signature = [0u8; 8];
        reader.read_exact(&mut signature)?;
        if &signature != TIS_SIGNATURE {
            error!("Not a TIS file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {:?}",
                signature
            )));
        }

        let tile_count = reader.read_u32()? as usize;
        let tile_length = reader.read_u32()?;
        if tile_length != PALETTE_TILE_LENGTH {
            return Err(std::io::Error::other(format!(
                "Invalid TIS palette tile_length: {tile_length} (expected {PALETTE_TILE_LENGTH})"
            )));
        }
        let _header_size = reader.read_u32()?;
        let tile_dimension = reader.read_u32()?;
        if tile_dimension != TILE_DIMENSION {
            // Vanilla games and NearInfinity treat anything else as
            // malformed; bail rather than silently producing garbage.
            return Err(std::io::Error::other(format!(
                "Invalid TIS tile_dimension: {tile_dimension} (expected {TILE_DIMENSION})"
            )));
        }

        let pixels_per_tile = (tile_dimension * tile_dimension) as usize;
        let mut tiles = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            // 1. Palette — 256 BGRA entries.
            let mut palette = Vec::with_capacity(256);
            for _ in 0..256 {
                let b = reader.read_u8()?;
                let g = reader.read_u8()?;
                let r = reader.read_u8()?;
                let alpha = reader.read_u8()?;
                palette.push(Rgb { r, g, b, alpha });
            }
            // 2. Pixel data — 64×64 palette indices.
            let mut pixel_palette_indexes = vec![0u8; pixels_per_tile];
            reader.read_exact(&mut pixel_palette_indexes)?;
            tiles.push(TisPaletteTile {
                palette,
                pixel_palette_indexes,
            });
        }

        debug!(
            "Loaded TIS palette: {} tiles × {}x{}",
            tile_count, tile_dimension, tile_dimension
        );
        Ok(TisPalette {
            r#type: Type::Palette,
            tile_dimension,
            tiles,
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    use super::*;

    #[test]
    fn test_parse_palette_should_fail_if_wrong_signature() {
        let data = DataSource::new(get_assets_path().join("TIS/Pvrz/AR0107.tis"));
        let mut reader = data.reader().unwrap();
        // PVRZ TIS shares the signature but mismatches tile_length, which
        // surfaces as a different (still-error) message.
        let res = TisPaletteParser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_palette_fire01() {
        // FIRE01.tis (PST): single 64×64 tile, 5144 bytes total.
        let data = DataSource::new(get_assets_path().join("TIS/Palette/FIRE01.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPaletteParser::import(&mut reader).unwrap();

        assert_eq!(tis.r#type, Type::Palette);
        assert_eq!(tis.tile_dimension, 64);
        assert_eq!(tis.tiles.len(), 1);
        let tile = &tis.tiles[0];
        assert_eq!(tile.palette.len(), 256);
        assert_eq!(tile.pixel_palette_indexes.len(), 64 * 64);
    }

    #[test]
    fn test_parse_palette_hotcoalr() {
        // HOTCOALR.tis (BG:EE): 6 tiles, 30744 bytes total.
        let data = DataSource::new(get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPaletteParser::import(&mut reader).unwrap();

        assert_eq!(tis.tiles.len(), 6);
        for (i, tile) in tis.tiles.iter().enumerate() {
            assert_eq!(tile.palette.len(), 256, "tile {i} palette");
            assert_eq!(tile.pixel_palette_indexes.len(), 64 * 64, "tile {i} pixels");
        }
    }

    #[test]
    fn test_tile_image_dimensions() {
        let data = DataSource::new(get_assets_path().join("TIS/Palette/FIRE01.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPaletteParser::import(&mut reader).unwrap();
        let img = tis.tile_image(0);
        assert_eq!(img.width(), 64);
        assert_eq!(img.height(), 64);
        // Every pixel must have alpha = 255 unless palette[0] is the
        // magic green — FIRE01's palette[0] is RGB(5, 3, 4), so no
        // transparent pixels.
        let p0 = tis.tiles[0].palette[0];
        assert!(!(p0.r == 0 && p0.g == 255 && p0.b == 0));
        for px in img.pixels() {
            assert_eq!(px.0[3], 255);
        }
    }

    #[test]
    fn test_fire01_matches_png() {
        // Reference PNG was exported externally and is 64×64 RGBA —
        // i.e. the single 64×64 tile laid out as a 1×1 grid.
        let data = DataSource::new(get_assets_path().join("TIS/Palette/FIRE01.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPaletteParser::import(&mut reader).unwrap();
        let actual = tis.to_image(1);
        assert_eq!(actual.width(), 64);
        assert_eq!(actual.height(), 64);
        assert_images_are_equal(
            &image::open(get_assets_path().join("TIS/Palette/FIRE01.png")).unwrap(),
            &actual.into(),
            None,
        );
    }

    #[test]
    fn test_hotcoalr_matches_png() {
        // Reference PNG is 384×64 — six 64×64 tiles laid out as a 6×1
        // horizontal strip. `to_image(6)` must reproduce it byte-exactly.
        let data = DataSource::new(get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPaletteParser::import(&mut reader).unwrap();
        let actual = tis.to_image(6);
        assert_eq!(actual.width(), 384);
        assert_eq!(actual.height(), 64);
        assert_images_are_equal(
            &image::open(get_assets_path().join("TIS/Palette/HOTCOALR.png")).unwrap(),
            &actual.into(),
            None,
        );
    }

    #[test]
    fn test_to_image_pads_short_last_row() {
        // 6 tiles in a 4-column grid → 2 rows, with the second row
        // half-filled. The unused (col 2 / col 3 of row 1) tile slots
        // must stay fully transparent.
        let data = DataSource::new(get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPaletteParser::import(&mut reader).unwrap();
        let img = tis.to_image(4);
        assert_eq!(img.width(), 256);
        assert_eq!(img.height(), 128);
        // Pixel at the top-left of the padded slot (col 2 of row 1):
        // (col=2, row=1) → (128, 64). Must be RGBA(0, 0, 0, 0).
        let padded = img.get_pixel(128, 64);
        assert_eq!(padded.0, [0, 0, 0, 0]);
    }
}
