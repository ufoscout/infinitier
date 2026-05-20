#![doc = include_str!("../readme.md")]

pub mod common;
mod importer;

pub use importer::palette::{TisPalette, TisPaletteTile};
pub use importer::pvrz::{TisPvrz, TisPvrzTile};
pub use importer::{TisImporter, detect_tis_type};

/// Shared TIS signature. Both palette- and PVRZ-backed variants use the
/// same on-disk magic; the variant is discriminated by the `tile_length`
/// field at offset `0x0C` (see [`detect_tis_type`]).
pub const TIS_SIGNATURE: &[u8; 8] = b"TIS V1  ";

/// Tile side length in pixels. The IESDP / Enhanced Edition games hard-code
/// 64×64; anything else is rejected as malformed.
pub const TILE_DIMENSION: u32 = 64;

/// On-disk per-tile byte size for the palette variant: 1024 BGRA palette +
/// 4096 paletted pixels (64×64).
pub const PALETTE_TILE_LENGTH: u32 = 1024 + (TILE_DIMENSION * TILE_DIMENSION);

/// On-disk per-tile byte size for the PVRZ variant: a 12-byte
/// `{page: i32, x: u32, y: u32}` block referencing a PVRZ page.
pub const PVRZ_TILE_LENGTH: u32 = 12;

/// Sentinel value stored in [`TisPvrzTile::pvrz_page`] when a tile has no
/// source PVRZ — NearInfinity fills the corresponding tile area with
/// black when rendering. Matches the on-disk `0xFFFFFFFF`.
pub const TIS_PVRZ_NO_SOURCE: i32 = -1;

/// Recognized TIS variants. Both share the same `TIS V1  ` signature on
/// disk; the variant is determined by the `tile_length` header field.
/// Naming follows NearInfinity (`Type.PALETTE` / `Type.PVRZ`).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    /// Legacy paletted tiles. Each tile carries its own 256-entry BGRA
    /// palette followed by a 64×64 index array.
    Palette,
    /// Enhanced-Edition atlas tiles — each tile is a 12-byte rectangle
    /// reference into a separate PVRZ page.
    Pvrz,
}

/// A parsed TIS file. Mirrors the [`Mos`](infinitier_datasource) split:
/// the dispatcher returns one of two variants based on the on-disk
/// `tile_length` field (see [`detect_tis_type`]).
#[derive(Debug, PartialEq, Eq)]
pub enum Tis {
    Palette(TisPalette),
    Pvrz(TisPvrz),
}
