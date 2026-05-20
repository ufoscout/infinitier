#![doc = include_str!("../readme.md")]
//!
//! ## Format
//!
//! Mirrors NearInfinity's `FntResource.java`:
//!
//! ```text
//! 0   # extra letters   u32     count of glyphs in the companion BMP
//! 4   <opaque body>            engine-internal; NI doesn't parse this
//! ```
//!
//! The `Letters` BAM and `Extra letters` BMP that NI shows in its
//! viewer are **synthesised from the FNT's own resource name** — they
//! aren't stored in the file. `DIALOG.FNT` ⇒ `DIALOG.BAM` (standard
//! glyphs) and `DIALOG.BMP` (extra glyphs).
//!
//! Everything past the 4-byte header is left as opaque bytes here for
//! the same reason it is in NI: the format is otherwise undocumented
//! and shipping a half-right renderer would be worse than not
//! rendering at all.
//!
//! ## Reverse-engineering notes (not parsed)
//!
//! For a while this crate exposed a parsed view of the body. The
//! parsing was correct as far as it went, but **no consumer needed
//! the data** — gemrb ignores the entire file and NearInfinity ignores
//! everything past offset 4 — so the parsing was reverted. The
//! findings are recorded here so the work isn't lost; if a future use
//! case appears (e.g. an FNT→TTF converter or a glyph-cache exporter),
//! the structure below is reproducible and well-defined.
//!
//! ### Extended header (offsets 4..16)
//!
//! ```text
//! 4   u16   num_sizes               # of pre-rendered font sizes
//! 6   u16   constant 1              always 1 in every sample
//! 8   u32   constant 1              always 1 in every sample
//! 12  u32   unknown                 varies per font; semantics unclear
//! ```
//!
//! ### Body layout (offsets 16..)
//!
//! ```text
//! 16  u32[extra_letters_count]      character coverage table
//!     f32[num_sizes][4]             per-size font metrics — see below
//!     f32[extra_letters_count][4]   per-glyph float quadruplets,
//!                                   semantics undetermined (NOT the BAM
//!                                   frame dimensions — conjecture
//!                                   refuted)
//!     <remaining bytes>             pre-rasterised glyph coverage
//!                                   masks at the larger sizes (the
//!                                   companion BAM only carries the
//!                                   smallest size). Per-glyph stride
//!                                   isn't cleanly divisible; full
//!                                   layout would need engine-side
//!                                   instrumentation to nail down.
//! ```
//!
//! ### Per-size metrics — verified
//!
//! Each row of the size table is four floats:
//! `(em_size, ascent, line_height, descent)`. Across 8/8 sampled
//! BG:EE / IWD:EE / BG2:EE fonts the count matches `num_sizes`
//! exactly, with all later quadruplets zeroed:
//!
//! | Font     | `num_sizes` | non-zero quads |
//! |----------|-------------|----------------|
//! | NORMAL   | 11          | 11             |
//! | FLOATTXT | 11          | 11             |
//! | TOOLFONT | 8           | 8              |
//! | TOOLTIP  | 3           | 3              |
//! | STONEBIG | 3           | 3              |
//! | STATES2  | 3           | 3              |
//! | REALMS   | 4           | 4              |
//! | INITIALS | 1           | 1              |
//!
//! Values look reasonable: NORMAL's 11-entry ladder runs from
//! em = 9 px (ascent 11, descent 3) up to em = 37.5 px (ascent 47,
//! descent 12) — the classic EE UI font ladder. `ascent` equals
//! `line_height` in every observed entry across every observed font.
//!
//! ### BAM correlation — refuted
//!
//! The per-glyph float quadruplets immediately after the size table do
//! **not** echo the companion BAM's frame dimensions. Going through
//! the BAM's cycle table (each BAM-font cycle indexes by char code,
//! pointing at one frame holding that glyph), real glyph dims are e.g.
//! `'space'→(4,10)`, `'!'→(4,4)`, `'#'→(7,13)`, `'$'→(10,9)`. The
//! corresponding FNT quadruplets — `(0,1,2,0)`, `(1,2,1,0)`,
//! `(1,5,0,0)`, `(0,6,0,0)` — don't carry any of those numbers in any
//! column. Likely per-glyph X/Y offsets or kerning adjustments, but
//! couldn't be pinned down to a reproducible meaning from sample
//! correlation alone.

use std::io::Read;
use std::sync::Arc;

use infinitier_datasource::{DataSource, Importer, ReadExt};
use log::debug;

/// Byte length of the parsed FNT header — just `# extra letters`.
pub const HEADER_LEN: usize = 4;

/// An FNT bitmap-font importer.
///
/// `name` is the resource name (lowercase, no extension — matches
/// [`GameData`](infinitier_datasource)'s indexing convention). It's
/// used to synthesise the companion BAM/BMP filenames the way NI does.
pub struct FntImporter<'a> {
    pub name: &'a str,
}

impl Importer for FntImporter<'_> {
    type T = Fnt;

    fn import(&self, source: &DataSource) -> std::io::Result<Self::T> {
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

        let mut reader = std::io::Cursor::new(&raw[..HEADER_LEN]);
        let extra_letters_count = reader.read_u32()?;

        // Companion file names. Vanilla game-data uses uppercase
        // 8.3 forms (DIALOG.BAM, DIALOG.BMP) — match that for display
        // and BIF lookups, even though our own GameData index is
        // lowercased.
        let display_name = self.name.to_ascii_uppercase();
        let letters_bam = format!("{display_name}.BAM");
        let extra_letters_bmp = format!("{display_name}.BMP");

        debug!(
            "Loaded {} [FNT]: # extra letters={}, Letters={}, Extra letters={}, body={} bytes",
            self.name,
            extra_letters_count,
            letters_bam,
            extra_letters_bmp,
            raw.len() - HEADER_LEN,
        );

        Ok(Fnt {
            extra_letters_count,
            letters_bam,
            extra_letters_bmp,
            raw: Arc::new(raw),
        })
    }
}

/// A parsed FNT.
///
/// Matches the three fields NI surfaces in its `FntResource` viewer:
/// the 4-byte `# extra letters` count plus two synthesised resource
/// references to the companion BAM (standard glyphs) and BMP (extra
/// glyphs).
#[derive(Debug, Clone)]
pub struct Fnt {
    /// Number of extra letters stored in the companion BMP. Read from
    /// the first 4 bytes of the file.
    pub extra_letters_count: u32,
    /// Name of the companion BAM holding the standard glyphs —
    /// `<resource_name>.BAM`, synthesised from the FNT's own name.
    pub letters_bam: String,
    /// Name of the companion BMP holding the extra glyphs —
    /// `<resource_name>.BMP`, synthesised from the FNT's own name.
    pub extra_letters_bmp: String,
    /// Original file bytes — kept so consumers (e.g. the explorer
    /// viewer) can hex-dump the opaque post-header body.
    pub raw: Arc<Vec<u8>>,
}

impl Fnt {
    /// Slice of the file past the parsed 4-byte header. Engine-
    /// internal; not interpreted here. See the module docs for what's
    /// known about the structure.
    pub fn body(&self) -> &[u8] {
        &self.raw[HEADER_LEN..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::get_assets_path;

    #[test]
    fn test_parse_realms_fnt() {
        let path = get_assets_path().join("FNT/REALMS.fnt");
        let fnt = FntImporter { name: "realms" }
            .import(&DataSource::new(path))
            .unwrap();
        assert_eq!(fnt.extra_letters_count, 244);
        assert_eq!(fnt.letters_bam, "REALMS.BAM");
        assert_eq!(fnt.extra_letters_bmp, "REALMS.BMP");
    }

    #[test]
    fn test_companion_refs_use_uppercase_name() {
        // GameData stores names lowercased — the importer must
        // uppercase them when building the companion refs so the
        // display matches NI's (`DIALOG.BAM`, not `dialog.BAM`).
        let path = get_assets_path().join("FNT/REALMS.fnt");
        let fnt = FntImporter { name: "Realms" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(fnt.letters_bam, "REALMS.BAM");
        assert_eq!(fnt.extra_letters_bmp, "REALMS.BMP");
    }

    #[test]
    fn test_rejects_truncated_file() {
        // Less than 4 bytes can't even hold the count → UnexpectedEof.
        let data = DataSource::new(&[0u8; 3][..]);
        let err = FntImporter { name: "tiny" }.import(&data).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_accepts_exactly_4_bytes() {
        // A 4-byte FNT (just the header, no body) is structurally
        // valid — NI would happily display "# extra letters" and the
        // two synthesised refs with no body bytes. body() returns an
        // empty slice.
        let data = DataSource::new(&[0x05, 0x00, 0x00, 0x00][..]);
        let fnt = FntImporter { name: "stub" }.import(&data).unwrap();
        assert_eq!(fnt.extra_letters_count, 5);
        assert_eq!(fnt.letters_bam, "STUB.BAM");
        assert_eq!(fnt.body().len(), 0);
    }

    #[test]
    fn test_raw_matches_source() {
        let path = get_assets_path().join("FNT/REALMS.fnt");
        let expected = std::fs::read(&path).unwrap();
        let fnt = FntImporter { name: "realms" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(fnt.raw.as_slice(), expected.as_slice());
        assert_eq!(fnt.body().len(), expected.len() - HEADER_LEN);
    }
}
