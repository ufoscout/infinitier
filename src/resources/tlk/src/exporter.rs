//! TLK file writer.
//!
//! Round-trip semantics: re-importing the exported bytes yields a
//! [`Tlk`] value that is **struct-equal** to the source. For TLKs
//! produced by every shipped IE engine the round-trip is also
//! **byte-exact** because:
//!
//! 1. The header is rewritten verbatim from the parsed fields
//!    (signature, version, language id, entry count, strings
//!    offset).
//! 2. Each [`TlkEntry`] is rewritten verbatim — including the
//!    documented-as-unused `volume_variance` / `pitch_variance`
//!    fields and any engine-defined flag bits.
//! 3. The strings section is copied byte-for-byte from
//!    [`Tlk::strings`]; per-entry `(string_offset, string_length)`
//!    cursors are not regenerated, so caller edits that leave the
//!    cursors intact survive round-trip exactly.
//!
//! The strings-section offset (header 0x0E) is recomputed from the entry
//! count — the canonical `0x12 + entries.len() * 26` every shipped IE
//! engine uses — rather than reused from the parsed file, so adding or
//! removing entries can never desync it. (Byte-exactness still holds for
//! the canonical files; a hypothetical padded-gap input would lose its
//! gap.)

use std::io::{self, BufWriter, Write};
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use log::debug;

use crate::{ENTRY_LEN, HEADER_LEN, TLK_SIGNATURE, Tlk, TlkEntry};

/// File writer for TLK V1 resources.
pub struct TlkExporter;

impl TlkExporter {
    /// Serialises `tlk` to the on-disk byte stream.
    pub fn export<W: Write>(&self, tlk: &Tlk, writer: &mut W) -> io::Result<()> {
        let bytes = serialize(tlk)?;
        writer.write_all(&bytes)
    }

    /// Writes `tlk` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, tlk: &Tlk, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(tlk, &mut writer)?;
        writer.flush()
    }
}

fn serialize(tlk: &Tlk) -> io::Result<Vec<u8>> {
    let n_entries = tlk.entries.len();
    // Recompute the strings-section offset from the entry count — the
    // canonical layout (strings directly after the entries block) — so
    // adding/removing entries can never desync a stored offset.
    let strings_offset = HEADER_LEN + n_entries * ENTRY_LEN;
    let file_size = strings_offset + tlk.strings.len();
    let mut buf = vec![0u8; file_size];

    // Header
    buf[0x00..0x04].copy_from_slice(TLK_SIGNATURE);
    buf[0x04..0x08].copy_from_slice(tlk.version.as_bytes());
    buf[0x08..0x0A].copy_from_slice(&tlk.language_id.to_le_bytes());
    buf[0x0A..0x0E].copy_from_slice(&(n_entries as u32).to_le_bytes());
    buf[0x0E..0x12].copy_from_slice(&(strings_offset as u32).to_le_bytes());

    // Entries
    for (i, entry) in tlk.entries.iter().enumerate() {
        let off = HEADER_LEN + i * ENTRY_LEN;
        write_entry(&mut buf[off..off + ENTRY_LEN], entry);
    }

    // Strings section — copied verbatim, so entries that haven't
    // been touched land at exactly the byte position they came from.
    buf[strings_offset..strings_offset + tlk.strings.len()].copy_from_slice(&tlk.strings);

    debug!(
        "Wrote TLK V1: lang={}, {} entries, strings_section={} B, total={} B",
        tlk.language_id,
        n_entries,
        tlk.strings.len(),
        file_size,
    );

    Ok(buf)
}

fn write_entry(out: &mut [u8], entry: &TlkEntry) {
    debug_assert_eq!(out.len(), ENTRY_LEN);
    out[0x00..0x02].copy_from_slice(&entry.flags.to_le_bytes());
    write_resref(&mut out[0x02..0x0A], &entry.sound_resref);
    out[0x0A..0x0E].copy_from_slice(&entry.volume_variance.to_le_bytes());
    out[0x0E..0x12].copy_from_slice(&entry.pitch_variance.to_le_bytes());
    out[0x12..0x16].copy_from_slice(&entry.string_offset.to_le_bytes());
    out[0x16..0x1A].copy_from_slice(&entry.string_length.to_le_bytes());
}

/// Encode `s` via WINDOWS-1252 (the IE-wide single-byte encoding)
/// and copy the leading bytes into `out`, padding trailing bytes
/// with `\0`. WINDOWS-1252 is bijective for every byte value
/// 0x00..=0xFF, so the round-trip is byte-exact against the matching
/// `Reader::read_string` decode in the importer.
fn write_resref(out: &mut [u8], s: &str) {
    let (encoded, _, _) = WINDOWS_1252.encode(s);
    let n = encoded.len().min(out.len());
    out[..n].copy_from_slice(&encoded[..n]);
    // Trailing bytes stay zero (caller's slice is already zeroed by
    // the parent `vec![0u8; …]`).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TlkImporter;
    use crate::test_support::{all_tlk_fixtures, synth_tlk};
    use infinitier_datasource::{DataSource, Importer};

    #[test]
    fn round_trip_synthetic_bytes_are_exact() {
        let original = synth_tlk();
        let tlk = TlkImporter { name: "synth" }
            .import(&DataSource::new(original.clone()))
            .unwrap();
        let mut produced = Vec::new();
        TlkExporter.export(&tlk, &mut produced).unwrap();
        assert_eq!(
            produced, original,
            "synthetic TLK round-trip not byte-exact"
        );
    }

    /// Byte-exact round trip across the shipped corpus — every real
    /// `dialog.tlk` we ship must reproduce its source bytes exactly
    /// when imported and re-exported with no edits.
    #[test]
    fn corpus_round_trip_byte_exact() {
        let fixtures = all_tlk_fixtures();
        assert!(!fixtures.is_empty(), "no TLK fixtures discovered");
        for path in fixtures {
            let name = path.display().to_string();
            let original = std::fs::read(&path).unwrap();
            let tlk = TlkImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));
            let mut produced = Vec::new();
            TlkExporter
                .export(&tlk, &mut produced)
                .unwrap_or_else(|e| panic!("export {name}: {e}"));
            assert_eq!(
                produced.len(),
                original.len(),
                "size mismatch on round-trip for {name}",
            );
            assert!(
                produced == original,
                "byte-exact round-trip failed for {name}",
            );
        }
    }

    /// Struct-level round trip: import → export → import → expect
    /// identical entry / strings fields. Catches issues that a
    /// byte-exact compare wouldn't (e.g. an exporter that produces
    /// "different bytes that parse to the same struct").
    #[test]
    fn corpus_round_trip_struct_equal() {
        let fixtures = all_tlk_fixtures();
        assert!(!fixtures.is_empty(), "no TLK fixtures discovered");
        for path in fixtures {
            let name = path.display().to_string();
            let original = TlkImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));
            let mut produced = Vec::new();
            TlkExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {name}: {e}"));
            let re_imported = TlkImporter { name: &name }
                .import(&DataSource::new(produced))
                .unwrap_or_else(|e| panic!("re-import {name}: {e}"));
            assert_eq!(re_imported.version, original.version, "{name}: version");
            assert_eq!(
                re_imported.language_id, original.language_id,
                "{name}: language_id"
            );
            assert_eq!(re_imported.entries, original.entries, "{name}: entries");
            assert_eq!(re_imported.strings, original.strings, "{name}: strings");
        }
    }

    #[test]
    fn export_to_file_round_trip() {
        let original_bytes = synth_tlk();
        let tlk = TlkImporter { name: "rt_file" }
            .import(&DataSource::new(original_bytes.clone()))
            .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        TlkExporter.export_to_file(&tlk, tmp.path()).unwrap();
        let re_imported = TlkImporter { name: "rt_file2" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        assert_eq!(re_imported.entries, tlk.entries);
        assert_eq!(re_imported.strings, tlk.strings);
    }

    /// Adding an entry shifts the strings section; the recomputed
    /// strings offset keeps the file consistent (no stored offset to
    /// desync).
    #[test]
    fn added_entry_relayouts_strings_section() {
        let tlk = TlkImporter { name: "grow" }
            .import(&DataSource::new(synth_tlk()))
            .unwrap();
        let mut grown = TlkImporter { name: "grow2" }
            .import(&DataSource::new(synth_tlk()))
            .unwrap();
        grown.entries.push(grown.entries[0].clone());
        let mut bytes = Vec::new();
        TlkExporter.export(&grown, &mut bytes).unwrap();
        let re = TlkImporter { name: "grow3" }
            .import(&DataSource::new(bytes))
            .unwrap();
        assert_eq!(re.entries.len(), tlk.entries.len() + 1);
        assert_eq!(re.strings, grown.strings);
    }
}
