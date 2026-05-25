//! TLK file reader.
//!
//! Uses [`DataSource::preloaded_reader`] to pull the whole file into
//! memory once, then drives parsing with the encoding-aware
//! [`infinitier_datasource::Reader`] for random-access seeks,
//! NUL-trimmed resref reads, and little-endian integer reads.

use std::io::{Read, Seek};

use infinitier_datasource::{DataSource, Importer, ReadExt, SeekExt};
use log::debug;

use crate::{ENTRY_LEN, HEADER_LEN, TLK_SIGNATURE, Tlk, TlkEntry, TlkVersion};

/// File importer for TLK V1 resources.
///
/// The TLK format is engine-independent (the byte layout is the
/// same across every shipped IE game), so unlike e.g. the GAM
/// importer this struct carries no `engine` field.
pub struct TlkImporter<'a> {
    /// Caller-visible name for error / log messages — usually the
    /// dialog.tlk path.
    pub name: &'a str,
}

impl Importer for TlkImporter<'_> {
    type T = Tlk;

    fn import(&self, source: &DataSource) -> std::io::Result<Tlk> {
        let mut reader = source.preloaded_reader()?;
        let file_size = reader.seek(std::io::SeekFrom::End(0))?;
        if file_size < HEADER_LEN as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("TLK '{}' shorter than {HEADER_LEN}-byte header", self.name),
            ));
        }
        reader.set_position(0)?;
        let sig: [u8; 4] = reader.read_exact_to_array()?;
        if &sig != TLK_SIGNATURE {
            return Err(std::io::Error::other(format!(
                "Unsupported TLK signature in {}: {sig:?}",
                self.name
            )));
        }
        let ver: [u8; 4] = reader.read_exact_to_array()?;
        let version = match &ver {
            b"V1  " => TlkVersion::V1,
            _ => {
                return Err(std::io::Error::other(format!(
                    "Unsupported TLK version in {}: {ver:?}",
                    self.name
                )));
            }
        };
        let language_id = reader.read_u16()?;
        let num_entries = reader.read_u32()?;
        let strings_offset = reader.read_u32()?;

        let entries_end = HEADER_LEN as u64 + (num_entries as u64) * ENTRY_LEN as u64;
        if entries_end > file_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "TLK '{}': entries section [{HEADER_LEN}..{entries_end}] runs past file end ({file_size} B)",
                    self.name
                ),
            ));
        }
        if (strings_offset as u64) > file_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "TLK '{}': strings offset {strings_offset} past file end ({file_size} B)",
                    self.name
                ),
            ));
        }
        // The strings section must start at or after the end of the
        // entries block. We accept a gap (some toolchains pad here)
        // but reject overlap because it would corrupt either the
        // entry array or the string lookups.
        if (strings_offset as u64) < entries_end {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "TLK '{}': strings offset {strings_offset} overlaps the entries section (ends at {entries_end})",
                    self.name
                ),
            ));
        }

        let mut entries = Vec::with_capacity(num_entries as usize);
        for i in 0..num_entries as u64 {
            reader.set_position(HEADER_LEN as u64 + i * ENTRY_LEN as u64)?;
            let flags = reader.read_u16()?;
            let sound_resref = reader.read_string(8)?;
            let volume_variance = reader.read_u32()?;
            let pitch_variance = reader.read_u32()?;
            let string_offset = reader.read_u32()?;
            let string_length = reader.read_u32()?;
            entries.push(TlkEntry {
                flags,
                sound_resref,
                volume_variance,
                pitch_variance,
                string_offset,
                string_length,
            });
        }

        reader.set_position(strings_offset as u64)?;
        let strings_len = file_size.saturating_sub(strings_offset as u64) as usize;
        let mut strings = vec![0u8; strings_len];
        reader.read_exact(&mut strings)?;

        debug!(
            "Loaded {} [TLK V1]: lang={language_id}, {} entries, strings_section={} B",
            self.name,
            entries.len(),
            strings.len(),
        );

        Ok(Tlk::new(
            version,
            language_id,
            strings_offset,
            entries,
            strings,
            source.encoding(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{all_tlk_fixtures, synth_tlk};

    #[test]
    fn parse_and_lookup_synthetic() {
        let bytes = synth_tlk();
        let tlk = TlkImporter { name: "synth" }
            .import(&DataSource::new(bytes))
            .unwrap();
        assert_eq!(tlk.version, TlkVersion::V1);
        assert_eq!(tlk.language_id, 1252);
        assert_eq!(tlk.len(), 2);
        assert_eq!(tlk.get(0).as_deref(), Some("Hello"));
        assert_eq!(tlk.get(1).as_deref(), Some("World!"));
        // Out-of-range returns None, doesn't panic.
        assert_eq!(tlk.get(2), None);
        // Sentinel "no string" strref.
        assert_eq!(tlk.get(0xFFFF_FFFF), None);
    }

    #[test]
    fn rejects_wrong_signature() {
        let mut bytes = synth_tlk();
        bytes[0..4].copy_from_slice(b"BAD ");
        let err = TlkImporter { name: "bad" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported TLK signature"));
    }

    #[test]
    fn rejects_wrong_version() {
        let mut bytes = synth_tlk();
        bytes[4..8].copy_from_slice(b"V2  ");
        let err = TlkImporter { name: "v2" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported TLK version"));
    }

    #[test]
    fn rejects_strings_overlap_entries() {
        let mut bytes = synth_tlk();
        // Force strings_offset = 0 — that overlaps the header / entries.
        bytes[0x0E..0x12].copy_from_slice(&0u32.to_le_bytes());
        let err = TlkImporter { name: "overlap" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        assert!(err.to_string().contains("overlaps the entries section"));
    }

    /// Correctness: lookups against the shipped corpus produce
    /// plausible English text. A best-effort sanity check —
    /// `dialog.tlk` content is opaque ASCII strings, so we assert:
    /// (a) at least one strref decodes to a non-empty, mostly-ASCII
    /// string, and (b) the decoded text contains common English
    /// stop-words. Catches mis-aligned offset / length arithmetic
    /// that the conformance / round-trip tests can't see.
    #[test]
    fn corpus_correctness_lookup() {
        let fixtures = all_tlk_fixtures();
        assert!(!fixtures.is_empty(), "no TLK fixtures discovered");
        for path in fixtures {
            let name = path.display().to_string();
            let tlk = TlkImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));

            // Sample a few strrefs; we don't know which ones are
            // populated so we tolerate empties and just need the
            // running tally to exceed a threshold.
            let mut populated = 0usize;
            let mut total_chars = 0usize;
            let mut combined = String::new();
            let n = tlk.len() as u32;
            for strref in (0..n).step_by((n as usize / 200).max(1)) {
                let Some(s) = tlk.get(strref) else { continue };
                if s.is_empty() {
                    continue;
                }
                populated += 1;
                total_chars += s.chars().count();
                if combined.len() < 4000 {
                    combined.push(' ');
                    combined.push_str(&s);
                }
            }
            assert!(
                populated >= 10,
                "{name}: fewer than 10 of 200 sampled strrefs decoded to non-empty strings"
            );
            assert!(
                total_chars >= 200,
                "{name}: sampled strings total only {total_chars} chars — suspiciously short",
            );
            // Sanity-check for English-looking content. Every BG /
            // IWD / PST dialog.tlk has thousands of occurrences of
            // these common words; if none appear, the strings are
            // likely garbage from a parser misalignment.
            let lower = combined.to_lowercase();
            let common = ["the", "and", "you"];
            assert!(
                common.iter().any(|w| lower.contains(w)),
                "{name}: none of {common:?} appeared in {} sampled chars — strings likely misaligned",
                combined.len(),
            );
        }
    }

    /// Conformance: every shipped corpus fixture must parse with the
    /// expected header invariants — V1 signature, header length 18,
    /// each entry's `(offset, length)` window fits inside the
    /// strings section.
    #[test]
    fn corpus_conformance() {
        let fixtures = all_tlk_fixtures();
        assert!(!fixtures.is_empty(), "no TLK fixtures discovered");
        for path in fixtures {
            let name = path.display().to_string();
            let tlk = TlkImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));
            assert_eq!(tlk.version, TlkVersion::V1, "version mismatch for {name}");
            assert!(
                !tlk.is_empty(),
                "fixture {name} has zero entries — should never happen for a real dialog.tlk"
            );
            for (i, entry) in tlk.entries.iter().enumerate() {
                let start = entry.string_offset as usize;
                let end = start + entry.string_length as usize;
                assert!(
                    end <= tlk.strings.len(),
                    "{name}: entry #{i} window [{start}..{end}] runs past strings section ({} B)",
                    tlk.strings.len(),
                );
            }
        }
    }
}
