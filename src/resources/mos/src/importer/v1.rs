//! MOS V1 parser.
//!
//! Wire layout (mirrors NearInfinity's `MosV1Decoder`):
//!
//! ```text
//! 0    "MOS V1  "       (8 bytes)
//! 8    width            u16  (image pixel width)
//! 10   height           u16  (image pixel height)
//! 12   columns          u16  (block grid width, in blocks)
//! 14   rows             u16  (block grid height, in blocks)
//! 16   block_size       u32  (always 64 — block dimension in pixels)
//! 20   palette_offset   u32  (always 24 — start of the palette section)
//! 24   <palettes>       (columns*rows × 1024 bytes — one 256-entry BGRA palette per block)
//!      <lookup table>   (columns*rows × u32 — per-block data offset relative to the data section)
//!      <pixel data>     (variable — paletted pixel bytes for each block)
//! ```
//!
//! Each block paints a 64×64 region of the final image; the right-most
//! column and bottom-most row may be truncated to fit the overall width
//! and height. Blocks are stored row-major from top-left.

use std::io::{BufRead, Read, Seek};

use image::{ImageBuffer, Rgba};
use infinitier_datasource::{ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::common::Rgb;
use crate::{MOS_V1_SIGNATURE, Type};

/// Fixed MOS V1 block dimension — always 64 pixels per the IESDP and the
/// games. The on-disk `block_size` field is validated against this.
pub const BLOCK_DIMENSION: u32 = 64;

#[derive(Debug, PartialEq, Eq)]
pub struct MosV1 {
    /// `Type::MosV1` for plain files, `Type::Mosc` for files that arrived
    /// wrapped in a MOSC zlib envelope. The decoded content is identical
    /// in both cases; this tag preserves the original wrapper so a future
    /// exporter can round-trip back to the source variant.
    pub r#type: Type,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of block columns; equals `ceil(width / 64)`.
    pub columns: u16,
    /// Number of block rows; equals `ceil(height / 64)`.
    pub rows: u16,
    /// Blocks in row-major order, top-left first. There are `columns * rows`
    /// of them; each block carries its own 256-entry palette and a
    /// `width × height` palette-index array.
    pub blocks: Vec<MosV1Block>,
}

impl MosV1 {
    /// Renders the MOS image into a single RGBA buffer.
    ///
    /// Blocks tile the canvas row-major from the top-left; each one paints
    /// a `64×64` region with its own palette (the rightmost column and
    /// bottom row may be truncated). Mirrors NearInfinity's MOS rendering
    /// with transparency enabled — palette alpha is ignored, pixels are
    /// opaque, and the magic color `RGB(0, 255, 0)` becomes fully
    /// transparent (the engine's "green-screen" convention; see [`Rgb`]).
    pub fn to_image(&self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let mut img = ImageBuffer::new(self.width, self.height);
        for (idx, block) in self.blocks.iter().enumerate() {
            let col = (idx as u32) % (self.columns as u32);
            let row = (idx as u32) / (self.columns as u32);
            let x0 = col * BLOCK_DIMENSION;
            let y0 = row * BLOCK_DIMENSION;
            for y in 0..block.height {
                for x in 0..block.width {
                    let i = (y * block.width + x) as usize;
                    let p = &block.palette[block.pixel_palette_indexes[i] as usize];
                    let alpha = if p.r == 0 && p.g == 255 && p.b == 0 {
                        0
                    } else {
                        255
                    };
                    img.put_pixel(x0 + x, y0 + y, Rgba([p.r, p.g, p.b, alpha]));
                }
            }
        }
        img
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MosV1Block {
    /// Block width in pixels — always 64 except for the last column,
    /// which may be narrower to fit the image's overall width.
    pub width: u32,
    /// Block height in pixels — always 64 except for the last row.
    pub height: u32,
    /// Per-block 256-entry palette (BGRA on disk, stored as RGBA-ish).
    pub palette: Vec<Rgb>,
    /// `width * height` palette indices in row-major order.
    pub pixel_palette_indexes: Vec<u8>,
}

/// A MOS V1 file importer.
pub struct MosV1Parser;

impl MosV1Parser {
    /// Parses a MOS V1 archive from a reader positioned at the start of
    /// the file (i.e. at the `"MOS V1  "` signature).
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<MosV1> {
        let mut signature = [0u8; 8];
        reader.read_exact(&mut signature)?;
        if &signature != MOS_V1_SIGNATURE {
            error!("Not a MOS V1 file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {:?}",
                signature
            )));
        }

        let width = reader.read_u16()? as u32;
        let height = reader.read_u16()? as u32;
        let columns = reader.read_u16()?;
        let rows = reader.read_u16()?;
        let block_size = reader.read_u32()?;
        if block_size != BLOCK_DIMENSION {
            // Vanilla games and NearInfinity both treat anything else as a
            // malformed file; bail rather than silently producing garbage.
            return Err(std::io::Error::other(format!(
                "Invalid MOS V1 block size: {block_size} (expected {BLOCK_DIMENSION})"
            )));
        }
        let palette_offset = reader.read_u32()? as u64;

        if width == 0 || height == 0 || columns == 0 || rows == 0 {
            return Err(std::io::Error::other(format!(
                "Invalid MOS V1 dimensions: {width}x{height}, {columns} columns × {rows} rows"
            )));
        }

        let block_count = columns as usize * rows as usize;
        let lookup_offset = palette_offset + (block_count as u64) * 1024;
        let data_section_start = lookup_offset + (block_count as u64) * 4;

        // 1. Palettes (256 BGRA entries each, in block order).
        reader.set_position(palette_offset)?;
        let mut palettes: Vec<Vec<Rgb>> = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            let mut palette = Vec::with_capacity(256);
            for _ in 0..256 {
                let b = reader.read_u8()?;
                let g = reader.read_u8()?;
                let r = reader.read_u8()?;
                let alpha = reader.read_u8()?;
                palette.push(Rgb { r, g, b, alpha });
            }
            palettes.push(palette);
        }

        // 2. Lookup table (one u32 per block, offset into the data section).
        reader.set_position(lookup_offset)?;
        let mut lookup: Vec<u32> = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            lookup.push(reader.read_u32()?);
        }

        // 3. Per-block pixel data. Block dimensions are derived from the
        //    grid position: every block is 64×64 except the right-most
        //    column and bottom-most row, which truncate to fit width and
        //    height exactly.
        let mut blocks: Vec<MosV1Block> = Vec::with_capacity(block_count);
        for (idx, palette) in palettes.into_iter().enumerate() {
            let col = (idx as u32) % (columns as u32);
            let row = (idx as u32) / (columns as u32);
            let block_width = if col + 1 < columns as u32 {
                BLOCK_DIMENSION
            } else {
                width - col * BLOCK_DIMENSION
            };
            let block_height = if row + 1 < rows as u32 {
                BLOCK_DIMENSION
            } else {
                height - row * BLOCK_DIMENSION
            };
            let pixel_count = block_width as usize * block_height as usize;

            reader.set_position(data_section_start + lookup[idx] as u64)?;
            let mut pixel_palette_indexes = vec![0u8; pixel_count];
            reader.read_exact(&mut pixel_palette_indexes)?;

            blocks.push(MosV1Block {
                width: block_width,
                height: block_height,
                palette,
                pixel_palette_indexes,
            });
        }

        debug!(
            "Loaded MOS V1: {}x{} ({} cols × {} rows = {} blocks)",
            width, height, columns, rows, block_count
        );
        Ok(MosV1 {
            r#type: Type::MosV1,
            width,
            height,
            columns,
            rows,
            blocks,
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    use super::*;

    #[test]
    fn test_parse_mos_v1_should_fail_if_wrong_signature() {
        let data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let mut reader = data.reader().unwrap();
        let res = MosV1Parser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_mos_v1_gtrspcap() {
        // GTRSPCAP.mos (BG): 5×5 image, single 5×5 block.
        let data = DataSource::new(get_assets_path().join("MOS/V1/GTRSPCAP.mos"));
        let mut reader = data.reader().unwrap();
        let mos = MosV1Parser::import(&mut reader).unwrap();

        assert_eq!(mos.r#type, Type::MosV1);
        assert_eq!(mos.width, 5);
        assert_eq!(mos.height, 5);
        assert_eq!(mos.columns, 1);
        assert_eq!(mos.rows, 1);
        assert_eq!(mos.blocks.len(), 1);

        // The single block fills the whole 5×5 image; its palette has 256
        // entries and its pixel array has 25 indices.
        let block = &mos.blocks[0];
        assert_eq!(block.width, 5);
        assert_eq!(block.height, 5);
        assert_eq!(block.palette.len(), 256);
        assert_eq!(block.pixel_palette_indexes.len(), 25);
    }

    #[test]
    fn test_mos_v1_gtrspcap_matches_png() {
        let data = DataSource::new(get_assets_path().join("MOS/V1/GTRSPCAP.mos"));
        let mut reader = data.reader().unwrap();
        let mos = MosV1Parser::import(&mut reader).unwrap();

        assert_images_are_equal(
            &image::open(get_assets_path().join("MOS/V1/GTRSPCAP.png")).unwrap(),
            &mos.to_image().into(),
            None,
        );
    }
}
