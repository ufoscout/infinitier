//! TIS PVRZ parser.
//!
//! Wire layout (mirrors NearInfinity's `TisV2Decoder`):
//!
//! ```text
//! 0    "TIS V1  "       (8 bytes — same signature as the palette variant)
//! 8    tile_count       u32
//! 12   tile_length      u32  (always 12 — distinguishes from the palette variant)
//! 16   header_size      u32  (always 24)
//! 20   tile_dimension   u32  (always 64)
//! 24   <tile entries>   (tile_count × 12 bytes)
//! ```
//!
//! Each tile entry is `(pvrz_page: i32, source_x: u32, source_y: u32)`,
//! a rectangle of `tile_dimension × tile_dimension` pixels copied from
//! the referenced PVRZ page into the tile slot. `pvrz_page == -1`
//! (`0xFFFFFFFF` on disk) is the "no source" sentinel; NearInfinity
//! paints the tile black in that case. Field order matches BAM V2 /
//! MOS V2's data block layout.

use std::io::{BufRead, Read, Seek};

use infinitier_datasource::{ReadExt, Reader};
use log::{debug, error};

use crate::{PVRZ_TILE_LENGTH, TILE_DIMENSION, TIS_PVRZ_NO_SOURCE, TIS_SIGNATURE, Type};

/// One TIS PVRZ tile — a reference to a 64×64 rectangle inside a
/// `<base><page:02d>.PVRZ` page. The base name is derived from the TIS
/// file name (see [`TisPvrz::pvrz_name_for`]).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TisPvrzTile {
    /// PVRZ page index. `-1` ([`TIS_PVRZ_NO_SOURCE`]) marks "no source"
    /// — NearInfinity renders this tile as a black square.
    pub pvrz_page: i32,
    /// X coordinate of the top-left source pixel inside the PVRZ page.
    pub source_x: u32,
    /// Y coordinate of the top-left source pixel inside the PVRZ page.
    pub source_y: u32,
}

impl TisPvrzTile {
    /// `true` if this tile has no PVRZ source (sentinel `-1`).
    pub fn is_blank(&self) -> bool {
        self.pvrz_page == TIS_PVRZ_NO_SOURCE
    }
}

/// A TIS PVRZ-variant archive (NearInfinity's `Type.PVRZ`).
///
/// Holds metadata only — actual pixel decoding requires opening the
/// referenced PVRZ files. Use [`pvrz_name_for`](Self::pvrz_name_for) to
/// derive each page's filename from the TIS resource name.
#[derive(Debug, PartialEq, Eq)]
pub struct TisPvrz {
    /// `Type::Pvrz`.
    pub r#type: Type,
    /// Tile side length in pixels (always 64).
    pub tile_dimension: u32,
    /// Tiles in file order. The `.wed` overlay map decides how they tile
    /// onto the area.
    pub tiles: Vec<TisPvrzTile>,
}

impl TisPvrz {
    /// Derives the PVRZ filename a given `page` resolves to, using the
    /// TIS resource name `tis_name` (e.g. `"AR0107"`).
    ///
    /// The engine convention (matching NearInfinity's
    /// `TisV2Decoder.pvrzNameBase`) drops the second character of the
    /// TIS resref and appends the zero-padded page index:
    ///
    /// ```text
    /// "AR0107" + page 0  -> "A010700.PVRZ"
    /// "AR0107" + page 7  -> "A010707.PVRZ"
    /// ```
    ///
    /// Returns `None` for the [`TIS_PVRZ_NO_SOURCE`] sentinel or for
    /// TIS names shorter than 2 characters.
    pub fn pvrz_name_for(tis_name: &str, page: i32) -> Option<String> {
        if page == TIS_PVRZ_NO_SOURCE {
            return None;
        }
        let page = u32::try_from(page).ok()?;
        let mut chars = tis_name.chars();
        let first = chars.next()?;
        let _drop = chars.next()?;
        let tail: String = chars.collect();
        Some(format!("{first}{tail}{:02}.PVRZ", page))
    }
}

/// A TIS PVRZ-variant file importer.
pub struct TisPvrzParser;

impl TisPvrzParser {
    /// Parses a TIS PVRZ archive from a reader positioned at the start
    /// of the file (i.e. at the `"TIS V1  "` signature).
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<TisPvrz> {
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
        if tile_length != PVRZ_TILE_LENGTH {
            return Err(std::io::Error::other(format!(
                "Invalid TIS PVRZ tile_length: {tile_length} (expected {PVRZ_TILE_LENGTH})"
            )));
        }
        let _header_size = reader.read_u32()?;
        let tile_dimension = reader.read_u32()?;
        if tile_dimension != TILE_DIMENSION {
            return Err(std::io::Error::other(format!(
                "Invalid TIS tile_dimension: {tile_dimension} (expected {TILE_DIMENSION})"
            )));
        }

        let mut tiles = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            tiles.push(TisPvrzTile {
                pvrz_page: reader.read_i32()?,
                source_x: reader.read_u32()?,
                source_y: reader.read_u32()?,
            });
        }

        debug!(
            "Loaded TIS PVRZ: {} tiles × {}x{}",
            tile_count, tile_dimension, tile_dimension
        );
        Ok(TisPvrz {
            r#type: Type::Pvrz,
            tile_dimension,
            tiles,
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::get_assets_path;

    use super::*;

    #[test]
    fn test_parse_pvrz_should_fail_if_wrong_signature() {
        // Same-signature fixtures are rejected by the tile_length check;
        // here we just want to confirm a malformed signature is also caught.
        let data = DataSource::new(&b"GARBAGE!aaaaaaaaaaaaa"[..]);
        let mut reader = data.reader().unwrap();
        assert!(TisPvrzParser::import(&mut reader).is_err());
    }

    #[test]
    fn test_parse_pvrz_ar0107() {
        // AR0107.tis (BG:EE): 48 tile entries, 600 bytes total.
        let data = DataSource::new(get_assets_path().join("TIS/Pvrz/AR0107.tis"));
        let mut reader = data.reader().unwrap();
        let tis = TisPvrzParser::import(&mut reader).unwrap();

        assert_eq!(tis.r#type, Type::Pvrz);
        assert_eq!(tis.tile_dimension, 64);
        assert_eq!(tis.tiles.len(), 48);

        // Tile 0 is the `0xFFFFFFFF, 0, 0` "no source" sentinel we saw
        // in the raw hexdump.
        let t0 = tis.tiles[0];
        assert!(t0.is_blank());
        assert_eq!(t0.pvrz_page, TIS_PVRZ_NO_SOURCE);
        assert_eq!(t0.source_x, 0);
        assert_eq!(t0.source_y, 0);

        // At least one tile in the file must reference a real PVRZ page,
        // otherwise the whole tileset would be empty.
        assert!(tis.tiles.iter().any(|t| !t.is_blank()));
    }

    #[test]
    fn test_pvrz_name_derivation() {
        // Drop the 2nd char, append zero-padded page, suffix ".PVRZ".
        assert_eq!(
            TisPvrz::pvrz_name_for("AR0107", 0),
            Some("A010700.PVRZ".to_string())
        );
        assert_eq!(
            TisPvrz::pvrz_name_for("AR0107", 7),
            Some("A010707.PVRZ".to_string())
        );
        assert_eq!(
            TisPvrz::pvrz_name_for("AR0107", 99),
            Some("A010799.PVRZ".to_string())
        );
        // No-source sentinel → no filename.
        assert_eq!(TisPvrz::pvrz_name_for("AR0107", TIS_PVRZ_NO_SOURCE), None);
        // Too-short name → no filename rather than a panic.
        assert_eq!(TisPvrz::pvrz_name_for("A", 0), None);
    }
}
