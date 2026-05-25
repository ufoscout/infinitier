#![doc = include_str!("../readme.md")]
//!
//! ## On-disk layout (V1, the only version IE games actually use)
//!
//! ```text
//!  0x00   8 bytes  Signature ('TLK V1  ')
//!  0x08   2        Language id (Windows code page)
//!  0x0A   4        Number of string entries
//!  0x0E   4        Absolute offset of the string-data section
//! ```
//!
//! Followed by `num_entries` × 26-byte entries:
//!
//! ```text
//!  0x00   2   Flags (bit 0 = has-text, bit 1 = sound, bit 2 = token,
//!             bit 3+ = engine-defined)
//!  0x02   8   Sound resref (`.WAV`)
//!  0x0A   4   Volume variance
//!  0x0E   4   Pitch variance
//!  0x12   4   Offset of string bytes (relative to the string-data
//!             section)
//!  0x16   4   Length of the string in bytes
//! ```
//!
//! Then the string section starts at the absolute offset stored in
//! the header — `num_entries` strings concatenated, each at the
//! offset / length declared by its entry.

use std::io::{Read, Seek};

use infinitier_datasource::{DataSource, Importer, ReadExt, SeekExt};
use log::debug;

const HEADER_LEN: u64 = 0x12;
const ENTRY_LEN: u64 = 26;

/// 4-byte signature.
pub const TLK_SIGNATURE: &[u8; 4] = b"TLK ";
/// 4-byte version tag — only V1 is observed in shipped IE games.
pub const TLK_V1_TAG: &[u8; 4] = b"V1  ";

/// A loaded TLK file. Holds the parsed entry index plus the raw
/// string-bytes section; [`Tlk::get`] decodes individual strings on
/// demand to avoid eagerly allocating one `String` per entry
/// (`dialog.tlk` typically has 50–100k entries).
#[derive(Debug, Clone)]
pub struct Tlk {
    /// Windows code page identifier from header offset 0x08. Most
    /// shipped EE TLKs use `1252` (Western European); the Russian /
    /// Polish / Korean / Chinese builds use the appropriate Windows
    /// code page for their text.
    pub language_id: u16,
    /// One entry per strref — sound + offset/length metadata.
    pub entries: Vec<TlkEntry>,
    /// Raw bytes of the string section. Each entry's `(offset,
    /// length)` window into this buffer is decoded via the encoding
    /// configured on the caller's [`DataSource`] (default
    /// WINDOWS-1252, which matches every shipped TLK except the
    /// Asian / Cyrillic locales).
    pub strings: Vec<u8>,
    /// Encoding used to decode string bytes — copied from the
    /// [`DataSource`] at import time.
    encoding: &'static encoding_rs::Encoding,
}

/// One TLK entry — fixed 26-byte record on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlkEntry {
    /// Bit flags: bit 0 = has text, bit 1 = sound, bit 2 = standard
    /// tokens (`<CHARNAME>` etc.). Engine-specific bits sit above.
    pub flags: u16,
    /// 8-byte ASCIIZ resref pointing at a `.WAV` if `flags & 2`.
    pub sound_resref: String,
    /// Volume variance for the attached sound (ignored when
    /// `flags & 2` is clear).
    pub volume_variance: u32,
    /// Pitch variance for the attached sound.
    pub pitch_variance: u32,
    /// Offset of this entry's string bytes inside [`Tlk::strings`].
    pub string_offset: u32,
    /// Number of bytes the entry's string occupies.
    pub string_length: u32,
}

impl Tlk {
    /// Look up the string at `strref`, decoded using the TLK's
    /// encoding. Returns `None` when `strref` is out of range (or
    /// the sentinel `0xFFFFFFFF`, which the engines use to mean "no
    /// string").
    pub fn get(&self, strref: u32) -> Option<String> {
        if strref == 0xFFFF_FFFF || strref as usize >= self.entries.len() {
            return None;
        }
        let entry = &self.entries[strref as usize];
        let start = entry.string_offset as usize;
        let end = start.saturating_add(entry.string_length as usize);
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
}

/// File importer for TLK V1 resources.
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
        if file_size < HEADER_LEN {
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
        if &ver != TLK_V1_TAG {
            return Err(std::io::Error::other(format!(
                "Unsupported TLK version in {}: {ver:?}",
                self.name
            )));
        }
        let language_id = reader.read_u16()?;
        let num_entries = reader.read_u32()?;
        let strings_offset = reader.read_u32()? as u64;

        let entries_end = HEADER_LEN + (num_entries as u64) * ENTRY_LEN;
        if entries_end > file_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "TLK '{}': entries section [{HEADER_LEN}..{entries_end}] runs past file end ({file_size} B)",
                    self.name
                ),
            ));
        }
        if strings_offset > file_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "TLK '{}': strings offset {strings_offset} past file end ({file_size} B)",
                    self.name
                ),
            ));
        }

        let mut entries = Vec::with_capacity(num_entries as usize);
        for i in 0..num_entries as u64 {
            reader.set_position(HEADER_LEN + i * ENTRY_LEN)?;
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

        reader.set_position(strings_offset)?;
        let strings_len = file_size.saturating_sub(strings_offset) as usize;
        let mut strings = vec![0u8; strings_len];
        reader.read_exact(&mut strings)?;

        debug!(
            "Loaded {} [TLK V1]: lang={language_id}, {} entries, strings_section={} B",
            self.name,
            entries.len(),
            strings.len(),
        );

        Ok(Tlk {
            language_id,
            entries,
            strings,
            encoding: source.encoding(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic TLK with two entries — "Hello" and "World!"
    /// — so we can validate the parser without depending on a real
    /// `dialog.tlk` in the workspace.
    fn synth_tlk() -> Vec<u8> {
        // strings section = "HelloWorld!" (no NUL separators, the
        // offsets / lengths in the entries delimit them).
        let strings = b"HelloWorld!";
        let header_len = 0x12usize;
        let entry_len = 26usize;
        let n_entries = 2u32;
        let strings_offset = header_len + entry_len * n_entries as usize;
        let mut buf = Vec::new();
        // Header
        buf.extend_from_slice(b"TLK V1  ");
        buf.extend_from_slice(&1252u16.to_le_bytes()); // language id
        buf.extend_from_slice(&n_entries.to_le_bytes());
        buf.extend_from_slice(&(strings_offset as u32).to_le_bytes());
        // Entry 0: "Hello" @ offset=0 length=5
        buf.extend_from_slice(&1u16.to_le_bytes()); // flags (has text)
        buf.extend_from_slice(b"\0\0\0\0\0\0\0\0"); // sound resref
        buf.extend_from_slice(&0u32.to_le_bytes()); // volume
        buf.extend_from_slice(&0u32.to_le_bytes()); // pitch
        buf.extend_from_slice(&0u32.to_le_bytes()); // string offset
        buf.extend_from_slice(&5u32.to_le_bytes()); // string length
        // Entry 1: "World!" @ offset=5 length=6
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(b"\0\0\0\0\0\0\0\0");
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&5u32.to_le_bytes());
        buf.extend_from_slice(&6u32.to_le_bytes());
        // Strings section
        assert_eq!(buf.len(), strings_offset);
        buf.extend_from_slice(strings);
        buf
    }

    #[test]
    fn test_parse_and_lookup_synthetic() {
        let bytes = synth_tlk();
        let tlk = TlkImporter { name: "synth" }
            .import(&DataSource::new(bytes))
            .unwrap();
        assert_eq!(tlk.len(), 2);
        assert_eq!(tlk.get(0).as_deref(), Some("Hello"));
        assert_eq!(tlk.get(1).as_deref(), Some("World!"));
        // Out-of-range returns None, doesn't panic.
        assert_eq!(tlk.get(2), None);
        // Sentinel "no string" strref.
        assert_eq!(tlk.get(0xFFFF_FFFF), None);
    }

    #[test]
    fn test_rejects_wrong_signature() {
        let mut bytes = synth_tlk();
        bytes[0..4].copy_from_slice(b"BAD ");
        let err = TlkImporter { name: "bad" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported TLK signature"));
    }

    #[test]
    fn test_rejects_wrong_version() {
        let mut bytes = synth_tlk();
        bytes[4..8].copy_from_slice(b"V2  ");
        let err = TlkImporter { name: "v2" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported TLK version"));
    }
}
