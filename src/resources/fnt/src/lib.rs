#![doc = include_str!("../readme.md")]
//!
//! ## Format note
//!
//! The Infinity Engine Enhanced Editions ship with `.fnt` files in a
//! **proprietary, undocumented bitmap font format** (Beamdog's pre-2.0
//! engine font). NearInfinity's `FntResource.java` parses a *different*
//! format (a 20-byte BAM/BMP-reference stub) — but every actual EE
//! game-data FNT this codebase has seen is the proprietary variant, so
//! that's what this importer targets.
//!
//! ### What we parse reliably
//!
//! - A 16-byte fixed header (glyph count + a few small unknown fields).
//! - A `glyph_count × u32` table of Unicode character codes covered by
//!   the font.
//!
//! ### What we currently *don't* parse
//!
//! The remainder of the file holds per-glyph metric quadruplets and
//! pixel/coverage data, both as IEEE-754 floats — an unusual choice
//! that suggests an SDF or coverage-mask representation. The exact
//! semantics are not documented anywhere I could find, and reverse
//! engineering them from samples produced inconsistent results, so the
//! raw bytes are exposed verbatim via [`Fnt::raw`] for hex inspection
//! by the viewer.

use std::io::{Cursor, Read};
use std::sync::Arc;

use infinitier_datasource::{DataSource, Importer, ReadExt};
use log::debug;

/// Byte length of the FNT fixed header (count + four unknown fields).
pub const HEADER_LEN: usize = 16;

/// An FNT bitmap-font importer.
pub struct FntImporter<'a> {
    pub name: &'a str,
}

impl Importer for FntImporter<'_> {
    type T = Fnt;

    fn import(&self, source: &DataSource) -> std::io::Result<Self::T> {
        // FNT files are small enough (≤100 KB in practice) that pulling
        // them into memory once is cheaper than seeking; the viewer
        // wants the full byte string anyway for the hex dump.
        let mut raw = Vec::new();
        source.reader()?.read_to_end(&mut raw)?;
        if raw.len() < HEADER_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "FNT '{}' is only {} bytes; needs at least {HEADER_LEN}",
                    self.name,
                    raw.len()
                ),
            ));
        }

        // `ReadExt` is auto-impl'd for every `Read`, so a plain Cursor
        // is enough — we never decode text from this stream.
        let mut reader = Cursor::new(&raw[..]);
        let glyph_count = reader.read_u32()?;
        let field_4 = reader.read_u16()?;
        let field_6 = reader.read_u16()?;
        let field_8 = reader.read_u32()?;
        let field_c = reader.read_u32()?;

        let codes_byte_len = (glyph_count as usize).saturating_mul(4);
        let codes_end = HEADER_LEN
            .checked_add(codes_byte_len)
            .ok_or_else(|| std::io::Error::other("glyph_count overflow"))?;
        if raw.len() < codes_end {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "FNT '{}' has {} bytes but glyph_count={} needs {} bytes for the codes table",
                    self.name,
                    raw.len(),
                    glyph_count,
                    codes_end
                ),
            ));
        }

        let mut character_codes = Vec::with_capacity(glyph_count as usize);
        for _ in 0..glyph_count {
            character_codes.push(reader.read_u32()?);
        }

        debug!(
            "Loaded {} [FNT]: glyph_count={}, body={} bytes",
            self.name,
            glyph_count,
            raw.len() - codes_end
        );

        Ok(Fnt {
            glyph_count,
            field_4,
            field_6,
            field_8,
            field_c,
            character_codes,
            body_offset: codes_end,
            raw: Arc::new(raw),
        })
    }
}

/// A parsed FNT bitmap font.
#[derive(Debug, Clone)]
pub struct Fnt {
    /// Number of glyphs in the font — equals the length of
    /// [`Fnt::character_codes`].
    pub glyph_count: u32,
    /// Header field at offset `0x04` (u16). Observed values 3, 4, 5,
    /// 11, 12 — varies by font; semantics unknown.
    pub field_4: u16,
    /// Header field at offset `0x06` (u16). Always observed as `1`.
    pub field_6: u16,
    /// Header field at offset `0x08` (u32). Always observed as `1`.
    pub field_8: u32,
    /// Header field at offset `0x0C` (u32). Variable; semantics unknown.
    pub field_c: u32,
    /// Unicode code points covered by this font, in file order. Vanilla
    /// fonts ship `[9, 10, 13, 32..=126, 160..=255, …]` — control whitespace,
    /// ASCII printables, Latin-1 supplement, and a handful of
    /// typographic extras (`…`, `₤`, `€`).
    pub character_codes: Vec<u32>,
    /// Byte offset where the (un-parsed) per-glyph metric + pixel data
    /// section begins. Always equals `HEADER_LEN + glyph_count * 4`.
    pub body_offset: usize,
    /// Original file bytes, kept so a viewer can hex-dump the
    /// un-parsed body without re-reading the source.
    pub raw: Arc<Vec<u8>>,
}

impl Fnt {
    /// The portion of the file holding per-glyph metrics + pixel /
    /// coverage data. Not parsed today — see the module doc comment.
    pub fn body(&self) -> &[u8] {
        &self.raw[self.body_offset..]
    }

    /// `true` when this character code is in the font's coverage table.
    pub fn covers(&self, code: u32) -> bool {
        self.character_codes.contains(&code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::get_assets_path;

    #[test]
    fn test_parse_realms_fnt() {
        // REALMS.fnt is a small-ish font (~30 KB) that we know covers
        // the standard 244-character set; great smoke test.
        let path = get_assets_path().join("FNT/REALMS.fnt");
        let fnt = FntImporter { name: "realms" }
            .import(&DataSource::new(path))
            .unwrap();

        assert_eq!(fnt.glyph_count, 244);
        assert_eq!(fnt.character_codes.len(), 244);
        // Vanilla EE fonts always start with the three control
        // whitespace chars and the space — independent of which fonts
        // happen to be in the corpus.
        assert_eq!(&fnt.character_codes[..4], &[9, 10, 13, 32]);
        assert!(fnt.field_6 == 1);
        assert!(fnt.field_8 == 1);
        // body_offset === 16 + glyph_count * 4
        assert_eq!(fnt.body_offset, 16 + 244 * 4);
    }

    #[test]
    fn test_covers_lookup() {
        let path = get_assets_path().join("FNT/REALMS.fnt");
        let fnt = FntImporter { name: "realms" }
            .import(&DataSource::new(path))
            .unwrap();

        assert!(fnt.covers(b'A' as u32));
        assert!(fnt.covers(b'z' as u32));
        assert!(fnt.covers(32));
        // 0x0001 is not in any vanilla font's coverage list.
        assert!(!fnt.covers(0x0001));
    }

    #[test]
    fn test_rejects_truncated_file() {
        // 8 bytes is well under HEADER_LEN — must error, not panic.
        let data = DataSource::new(&[0u8; 8][..]);
        let err = FntImporter { name: "tiny" }.import(&data).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_rejects_glyph_count_larger_than_file() {
        // Header declares 1_000_000 glyphs in a 16-byte file — codes
        // table would need 4 MB more; must surface as InvalidData.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_000_000u32.to_le_bytes()); // glyph_count
        bytes.extend_from_slice(&[0u8; 12]); // rest of header
        let err = FntImporter { name: "lies" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_raw_matches_source() {
        let path = get_assets_path().join("FNT/REALMS.fnt");
        let expected = std::fs::read(&path).unwrap();
        let fnt = FntImporter { name: "realms" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(fnt.raw.as_slice(), expected.as_slice());
        // body() is just the slice after the header + codes table.
        assert_eq!(fnt.body().len(), expected.len() - fnt.body_offset);
    }
}
