#![doc = include_str!("../readme.md")]
//!
//! ## On-disk layout (V1, the only version IE games actually use)
//!
//! All offsets are little-endian. The format is identical across
//! every shipped IE engine (BG, BG2, EE, IWD, IWD2, PST) — only the
//! interpretation of certain bits inside [`TlkEntry::flags`] varies
//! by game, and those bits are preserved verbatim. The importer /
//! exporter therefore do not need a [`infinitier_common::Engine`]
//! parameter; see `readme.md` for the discussion.
//!
//! ```text
//!  Header (18 bytes)
//!  0x00   4   Signature ('TLK ')
//!  0x04   4   Version   ('V1  ')
//!  0x08   2   Language id (Windows code page; e.g. 1252 / 1250)
//!  0x0A   4   Number of string entries
//!  0x0E   4   Absolute offset of the string-data section
//!
//!  Entries (26 bytes each, `entry_count` records)
//!  0x00   2   Flags (bit 0 = has-text, bit 1 = has-sound,
//!                    bit 2 = standard-token; bits 3+ engine-defined)
//!  0x02   8   Sound resref (`.WAV`, fixed-width ASCIIZ)
//!  0x0A   4   Volume variance (unused, preserved verbatim)
//!  0x0E   4   Pitch variance  (unused, preserved verbatim)
//!  0x12   4   String offset, relative to the strings section
//!  0x16   4   String length in bytes
//!
//!  Strings section
//!  String bytes, length = file_size - strings_offset. Strings are
//!  not separated by NUL terminators; each entry's `(offset, length)`
//!  window selects its bytes inside this buffer.
//! ```
//!
//! The importer round-trips the entries and the strings section
//! verbatim, so callers can mutate either side and the exporter
//! produces bytes the engine will accept (subject to the caller
//! keeping the `(offset, length)` cursors in sync).

mod exporter;
mod importer;

pub use exporter::TlkExporter;
pub use importer::TlkImporter;

/// 4-byte signature at offset `0x00` of every TLK file.
pub const TLK_SIGNATURE: &[u8; 4] = b"TLK ";

/// On-disk header length (signature + version + lang + count +
/// strings-offset = 18 bytes). Sub-sections start at this offset.
pub const HEADER_LEN: usize = 0x12;

/// On-disk size of one [`TlkEntry`] (flags + sound resref + variance
/// + offset + length).
pub const ENTRY_LEN: usize = 26;

/// Known on-disk version tags. Only [`TlkVersion::V1`] is actually
/// observed in shipped IE games — the V2 spec is a community draft
/// that no engine recognises. Round-trip-safe enum: any unknown 4-byte
/// tag is rejected at import time rather than carried as `Unknown(_)`,
/// so we never serialise back a value the engine wouldn't load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlkVersion {
    /// `V1  ` — the only version produced by any shipped IE engine.
    V1,
}

impl TlkVersion {
    /// The 4-byte tag stored at offset `0x04` of every TLK file.
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            TlkVersion::V1 => b"V1  ",
        }
    }
}

/// A loaded TLK file.
///
/// Carries the parsed entry index plus the raw strings section.
/// [`Tlk::get`] decodes individual strings on demand to avoid
/// eagerly allocating one `String` per entry — `dialog.tlk` typically
/// has 50–100 thousand entries.
///
/// Round-trip semantics: re-exporting then re-importing a `Tlk`
/// yields a struct-equal value. The exporter is also **byte-exact**
/// for any `Tlk` whose `entries[*].string_offset / string_length`
/// pairs still match the contents of [`Self::strings`] (i.e. when
/// the caller has not edited strings in a way that shifts other
/// entries' offsets); see [`TlkExporter`] for details.
#[derive(Debug, Clone)]
pub struct Tlk {
    /// On-disk version tag — currently always [`TlkVersion::V1`].
    pub version: TlkVersion,
    /// Windows code page identifier from header offset 0x08.
    /// Shipped EE TLKs use `1252` (Western European); the
    /// Russian / Polish / Korean / Chinese builds use the matching
    /// Windows code page.
    pub language_id: u16,
    /// Absolute file offset of the strings section as stored on disk
    /// at header offset 0x0E. Preserved verbatim so the exporter can
    /// reproduce non-canonical inputs (e.g. tools that leave a gap
    /// between the entries block and the strings block) byte-for-byte.
    /// For every shipped IE engine this equals the canonical value
    /// `0x12 + entries.len() * 26` (no gap). Callers who add or
    /// remove entries must update this field accordingly — the
    /// exporter rejects values that would overlap the entries block.
    pub strings_offset: u32,
    /// One entry per strref — sound + offset/length metadata.
    pub entries: Vec<TlkEntry>,
    /// Raw bytes of the strings section. Each entry's
    /// `(string_offset, string_length)` window into this buffer is
    /// decoded via [`Self::encoding`] when [`Self::get`] is called.
    pub strings: Vec<u8>,
    /// Encoding used to decode string bytes — copied from the
    /// [`infinitier_datasource::DataSource`] at import time. Almost
    /// always WINDOWS-1252; the Asian locale TLKs use their matching
    /// Windows code page.
    encoding: &'static encoding_rs::Encoding,
}

/// One TLK entry — fixed 26-byte record on disk.
///
/// Field-level semantics:
/// - `flags`: bit-0 = "text exists", bit-1 = "sound exists",
///   bit-2 = "standard token expansion" (only honoured by BG2 and
///   the Enhanced Editions per IESDP). Bits 3+ are engine-defined
///   and preserved verbatim.
/// - `sound_resref`: 8-byte ASCIIZ resref pointing at a `.WAV` (when
///   `flags & 2`); decoded via WINDOWS-1252.
/// - `volume_variance` / `pitch_variance`: documented as "unused" by
///   IESDP. Preserved verbatim so the round-trip stays byte-exact
///   even for tools / mods that store non-zero values here.
/// - `string_offset` / `string_length`: the `(offset, length)` window
///   into [`Tlk::strings`] for this entry's text. The offset is
///   relative to the start of the strings section, not the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlkEntry {
    /// Bit flags — see the struct-level docs.
    pub flags: u16,
    /// 8-byte ASCIIZ resref pointing at a `.WAV` if `flags & 2`.
    pub sound_resref: String,
    /// Volume variance for the attached sound. IESDP marks this
    /// "unused"; preserved verbatim for round-trip.
    pub volume_variance: u32,
    /// Pitch variance for the attached sound. IESDP marks this
    /// "unused"; preserved verbatim for round-trip.
    pub pitch_variance: u32,
    /// Offset of this entry's string bytes inside [`Tlk::strings`]
    /// (relative to the strings section, not the file).
    pub string_offset: u32,
    /// Number of bytes the entry's string occupies.
    pub string_length: u32,
}

/// Bit 0 of [`TlkEntry::flags`] — "this entry carries text".
pub const FLAG_HAS_TEXT: u16 = 0x0001;
/// Bit 1 of [`TlkEntry::flags`] — "this entry has an attached
/// sound".
pub const FLAG_HAS_SOUND: u16 = 0x0002;
/// Bit 2 of [`TlkEntry::flags`] — "this entry uses standard token
/// expansion" (`<CHARNAME>`, …). Per IESDP, only honoured by BG2
/// and the Enhanced Editions.
pub const FLAG_HAS_TOKEN: u16 = 0x0004;

/// Sentinel value used by the IE engines to mean "no string" when a
/// strref field is unset.
pub const NO_STRREF: u32 = 0xFFFF_FFFF;

impl Tlk {
    /// Look up the string at `strref`, decoded using the TLK's
    /// encoding. Returns `None` when `strref` is out of range, when
    /// the entry's `(offset, length)` window points outside the
    /// strings section, or when `strref == 0xFFFFFFFF` (the engine's
    /// "no string" sentinel).
    pub fn get(&self, strref: u32) -> Option<String> {
        if strref == NO_STRREF || strref as usize >= self.entries.len() {
            return None;
        }
        let entry = &self.entries[strref as usize];
        let start = entry.string_offset as usize;
        let end = start.checked_add(entry.string_length as usize)?;
        if end > self.strings.len() {
            return None;
        }
        let bytes = &self.strings[start..end];
        let (decoded, _, _) = self.encoding.decode(bytes);
        Some(decoded.into_owned())
    }

    /// Total number of indexed strrefs (including empty entries).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when the table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The [`encoding_rs::Encoding`] this TLK was loaded with. Stored
    /// on the [`infinitier_datasource::DataSource`] at import time —
    /// callers can override the default (WINDOWS-1252) for the Asian
    /// locale TLKs that ship in Korean / Chinese / Russian builds.
    pub fn encoding(&self) -> &'static encoding_rs::Encoding {
        self.encoding
    }

    /// Internal constructor used by the importer. Kept `pub(crate)`
    /// because the [`Self::encoding`] field is private — callers who
    /// want to mint a `Tlk` from scratch should go through the
    /// importer on a synthetic byte stream.
    pub(crate) fn new(
        version: TlkVersion,
        language_id: u16,
        strings_offset: u32,
        entries: Vec<TlkEntry>,
        strings: Vec<u8>,
        encoding: &'static encoding_rs::Encoding,
    ) -> Self {
        Self {
            version,
            language_id,
            strings_offset,
            entries,
            strings,
            encoding,
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::{ENTRY_LEN, FLAG_HAS_TEXT, HEADER_LEN, Tlk, TlkImporter};

    /// Build a synthetic TLK with two entries — "Hello" and "World!"
    /// — so we can validate parser / writer without depending on a
    /// real `dialog.tlk` in the workspace. Shared between
    /// importer.rs and exporter.rs tests.
    pub fn synth_tlk() -> Vec<u8> {
        let strings = b"HelloWorld!";
        let n_entries = 2u32;
        let strings_offset = HEADER_LEN + ENTRY_LEN * n_entries as usize;
        let mut buf = Vec::new();
        // Header
        buf.extend_from_slice(b"TLK V1  ");
        buf.extend_from_slice(&1252u16.to_le_bytes());
        buf.extend_from_slice(&n_entries.to_le_bytes());
        buf.extend_from_slice(&(strings_offset as u32).to_le_bytes());
        // Entry 0: "Hello" @ offset=0 length=5
        buf.extend_from_slice(&FLAG_HAS_TEXT.to_le_bytes());
        buf.extend_from_slice(b"\0\0\0\0\0\0\0\0");
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&5u32.to_le_bytes());
        // Entry 1: "World!" @ offset=5 length=6
        buf.extend_from_slice(&FLAG_HAS_TEXT.to_le_bytes());
        buf.extend_from_slice(b"\0\0\0\0\0\0\0\0");
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&5u32.to_le_bytes());
        buf.extend_from_slice(&6u32.to_le_bytes());
        assert_eq!(buf.len(), strings_offset);
        buf.extend_from_slice(strings);
        buf
    }

    /// Recursively collect every `.tlk` file under `assets/TLK/`.
    pub fn all_tlk_fixtures() -> Vec<PathBuf> {
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.eq_ignore_ascii_case("tlk"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        let root = get_assets_path().join("TLK");
        if root.is_dir() {
            visit(&root, &mut out);
        }
        out.sort();
        out
    }

    /// Import a specific fixture given its path relative to
    /// `assets/TLK/`. Exposed for follow-on test modules; the
    /// in-crate tests scan the corpus dynamically via
    /// [`all_tlk_fixtures`].
    #[allow(dead_code)]
    pub fn import_fixture(rel_path: &str) -> Tlk {
        let path = get_assets_path().join("TLK").join(rel_path);
        TlkImporter { name: rel_path }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("import {rel_path}: {e}"))
    }
}
