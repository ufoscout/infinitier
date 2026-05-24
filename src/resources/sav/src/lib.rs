#![doc = include_str!("../readme.md")]
//!
//! ## Format
//!
//! Mirrors the SAV V1 layout documented at
//! <https://gibberlings3.github.io/iesdp/file_formats/ie_formats/sav_v1.htm>
//! and the parser in NearInfinity's `IOHandler` /
//! `SavResourceEntry`:
//!
//! ```text
//! 0   "SAV V1.0"            (8 bytes)
//! 8   <entries>
//! ```
//!
//! Each entry is laid out as:
//!
//! ```text
//! +0   u32   filename_length      includes the trailing NUL byte
//! +4   N     filename             ASCIIZ; the last byte is `\0`
//! +4+N u32   uncompressed_size
//! +8+N u32   compressed_size  (M)
//! +12+N M    zlib stream          standard RFC 1950 deflate
//! ```
//!
//! Entries are stored back-to-back with no padding. All multi-byte
//! integers are little-endian — same convention as every other IE
//! resource format.

use infinitier_common::ResourceType;
use infinitier_datasource::DataSource;

mod exporter;
mod importer;

pub use exporter::SavExporter;
pub use importer::SavImporter;

/// 8-byte file signature — present at offset 0 of every SAV V1 file.
pub const SAV_V1_SIGNATURE: &[u8; 8] = b"SAV V1.0";

/// A parsed SAV file.
///
/// The game ships with pristine area / store / creature resources in
/// BIFs. As the player plays, the engine snapshots the mutable state
/// of every area they've visited (containers, dropped loot, door
/// states, killed/moved actors, merchant inventories, region triggers,
/// etc.) and stores those snapshots inside the SAV. On re-entry to an
/// area, the engine loads the SAV's copy of the resource (e.g.
/// `AR0100.ARE`) instead of the pristine BIF copy — that's how
/// persistence works in IE games.
///
/// The SAV is essentially a per-savegame override store where every
/// resource is stored in a [`SavEntry`]. The on-disk zlib compression
/// is handled by the importer/exporter; [`SavEntry::data`] always
/// holds the decompressed bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sav {
    /// Entries in file order. Original ordering preserved so
    /// [`SavExporter`] writes them back in the same sequence.
    pub entries: Vec<SavEntry>,
}

impl Sav {
    /// Returns the first entry whose filename matches `name`
    /// case-insensitively, or `None`. Case-insensitive match is the
    /// convention IE games use for resource lookups.
    pub fn find(&self, name: &str) -> Option<&SavEntry> {
        self.entries
            .iter()
            .find(|e| e.filename.eq_ignore_ascii_case(name))
    }
}

/// One entry inside a SAV archive — a header descriptor plus the
/// **decompressed** resource bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavEntry {
    /// Resource filename, e.g. `"AR0100.ARE"`. The trailing NUL byte
    /// from the on-disk record is stripped.
    pub filename: String,
    /// Resource type resolved from the filename's extension via
    /// [`ResourceType::from_extension`].
    pub r#type: ResourceType,
    /// Decompressed resource bytes.
    pub data: Vec<u8>,
}

impl SavEntry {
    /// Returns a [`DataSource`] over this entry's decompressed bytes.
    pub fn data_source(&self) -> DataSource {
        DataSource::new(self.data.clone())
    }
}

/// Shared helpers for the per-module test files (`importer::tests`,
/// `exporter::tests`, and `tests` below). Kept here so a single
/// definition serves every consumer.
#[cfg(test)]
pub(crate) mod test_support {
    use std::io::Write;

    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::{Sav, SavImporter};

    /// Imports the BG / `Save/888888888-Mission-Pack-Save/Baldur.sav`
    /// fixture. The other identical copy under `MPSave/` is used by
    /// the corpus walk in `exporter::tests`.
    pub fn baldur_sav() -> Sav {
        SavImporter { name: "Baldur" }
            .import(&DataSource::new(
                get_assets_path().join("SAV_GAM/bg/Save/888888888-Mission-Pack-Save/Baldur.sav"),
            ))
            .unwrap()
    }

    /// Synthesises a minimal 1-entry SAV with the given filename and
    /// plaintext payload. Used by tests that need a controlled input
    /// (unknown extensions, corrupt zlib streams, …) without
    /// depending on a corpus file.
    pub fn build_synthetic_sav(filename: &str, payload: &[u8]) -> Vec<u8> {
        let mut compressed = Vec::new();
        {
            let mut enc =
                flate2::write::ZlibEncoder::new(&mut compressed, flate2::Compression::default());
            enc.write_all(payload).unwrap();
            enc.finish().unwrap();
        }
        let mut name_z = filename.as_bytes().to_vec();
        name_z.push(0); // trailing NUL
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"SAV V1.0");
        bytes.extend_from_slice(&(name_z.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&name_z);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use infinitier_datasource::{DataSource, Importer, SeekExt};
    use infinitier_test_utils::get_assets_path;

    use super::*;
    use crate::test_support::*;

    #[test]
    fn test_find_is_case_insensitive() {
        let sav = baldur_sav();
        assert!(sav.find("AR0100.ARE").is_some());
        assert!(sav.find("ar0100.are").is_some());
        assert!(sav.find("Ar0100.Are").is_some());
        assert!(sav.find("nonexistent.are").is_none());
    }

    #[test]
    fn test_two_corpus_files_parse() {
        // Both extracted Baldur.sav copies must parse with the same
        // entry layout — they're byte-identical anyway, but this
        // catches breakage if either copy ever diverges.
        let a = baldur_sav();
        let b = SavImporter { name: "Baldur-mp" }
            .import(&DataSource::new(get_assets_path().join(
                "SAV_GAM/bg/MPSave/888888888-Mission-Pack-Save/Baldur.sav",
            )))
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_data_source_yields_decompressed_payload() {
        // Self-contained sanity check: a known plaintext goes through
        // SAV encode → import → data_source().reader() → read_to_end
        // and comes back unchanged.
        let plaintext = b"the quick brown fox jumps over the lazy dog";
        let sav = SavImporter { name: "synth" }
            .import(&DataSource::new(build_synthetic_sav("fox.txt", plaintext)))
            .unwrap();
        let ds = sav.entries[0].data_source();
        let mut reader = ds.reader().unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out.as_slice(), plaintext);
    }

    #[test]
    fn test_data_source_first_entry_starts_with_area_signature() {
        // Walk the SAV's first ARE entry through data_source() and
        // confirm the first 8 bytes of the inflated stream are the
        // standard "AREAV1.0" signature.
        let sav = baldur_sav();
        let entry = &sav.entries[0];
        assert_eq!(entry.r#type, ResourceType::Are);
        let ds = entry.data_source();
        let mut reader = ds.reader().unwrap();
        let mut head = [0u8; 8];
        reader.read_exact(&mut head).unwrap();
        assert_eq!(&head, b"AREAV1.0");
    }

    #[test]
    fn test_data_source_total_length_matches_entry_data() {
        // Reading the DataSource to EOF must produce exactly
        // `entry.data.len()` bytes — proves `data_source` doesn't
        // truncate or pad.
        let sav = baldur_sav();
        let entry = &sav.entries[0];
        let ds = entry.data_source();
        let mut reader = ds.reader().unwrap();
        let mut out = Vec::with_capacity(entry.data.len());
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out.len(), entry.data.len());
    }

    #[test]
    fn test_data_source_supports_seek() {
        // The returned DataSource must expose Seek (via MemorySource
        // → Cursor) — without that, callers can't use it with
        // importers that depend on `set_position`.
        let sav = baldur_sav();
        let entry = &sav.entries[0];
        let ds = entry.data_source();
        let mut reader = ds.reader().unwrap();
        // Skip past the 8-byte AREA signature, then read 4 bytes.
        reader.set_position(8).unwrap();
        let mut chunk = [0u8; 4];
        reader.read_exact(&mut chunk).unwrap();
        // Re-seek to 0 and confirm we're back at "AREA".
        reader.set_position(0).unwrap();
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig).unwrap();
        assert_eq!(&sig, b"AREA");
    }

    #[test]
    fn test_data_source_independent_per_call() {
        // Two `data_source()` calls on the same entry must each
        // produce a usable DataSource with identical content.
        let sav = baldur_sav();
        let entry = &sav.entries[0];
        let ds_a = entry.data_source();
        let ds_b = entry.data_source();
        let mut a = Vec::new();
        let mut b = Vec::new();
        ds_a.reader().unwrap().read_to_end(&mut a).unwrap();
        ds_b.reader().unwrap().read_to_end(&mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), entry.data.len());
    }
}
