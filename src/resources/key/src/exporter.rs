use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::{Key, ResourceEntry};

/// A KEY file exporter.
///
/// Writes a [`Key`] back to the KEY V1 binary format used by Infinity Engine
/// games. Section offsets, the BIF filename string table, and per-entry
/// `string_offset`s are recomputed from the in-memory model, so the output
/// may not be byte-identical to a file produced by other tools (or to the
/// source CHITIN.KEY) — but re-importing it yields an equal [`Key`].
///
/// Demo vs standard layout is chosen by the `file_size` field of the BIF
/// entries: if every entry has `file_size = None`, the BG1-Demo 8-byte
/// layout is emitted; if every entry has `file_size = Some(_)`, the
/// standard 12-byte layout is emitted. A mix returns an error.
pub struct KeyExporter;

const HEADER_SIZE: u32 = 24;
const BIF_ENTRY_STD: u32 = 12;
const BIF_ENTRY_DEMO: u32 = 8;
const RESOURCE_ENTRY_SIZE: u32 = 14;

impl KeyExporter {
    /// Writes `key` as KEY V1 bytes into `writer`.
    pub fn export<W: Write>(&self, key: &Key, writer: &mut W) -> io::Result<()> {
        let bytes = build_bytes(key)?;
        writer.write_all(&bytes)
    }

    /// Writes `key` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, key: &Key, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(key, &mut writer)?;
        writer.flush()
    }
}

fn build_bytes(key: &Key) -> io::Result<Vec<u8>> {
    let is_demo = detect_demo(key)?.unwrap_or_default();

    let bif_entry_size = if is_demo {
        BIF_ENTRY_DEMO
    } else {
        BIF_ENTRY_STD
    };

    let n_bifs = key.bif_entries.len() as u32;
    let n_resources = key.resource_entries.len() as u32;

    let bif_offset = HEADER_SIZE;
    // Strings come right after the BIF entries section. For the demo layout
    // the BG1-Demo detector keys off the first entry's `string_offset` being
    // exactly `bif_offset + n_bifs * 8`, so this ordering is load-bearing for
    // demo files. For standard files the layout is free and this same order
    // works fine.
    let strings_offset = bif_offset + n_bifs * bif_entry_size;

    // Pre-compute per-entry string offsets and the total string-table size.
    let mut string_offsets = Vec::with_capacity(key.bif_entries.len());
    let mut cursor = strings_offset;
    for entry in &key.bif_entries {
        string_offsets.push(cursor);
        // Each name is written as the stored bytes + a null terminator;
        // `string_length` includes the terminator (matches the importer's
        // `string_length - 1` read).
        let len = u32::try_from(entry.file_name.len()).map_err(|_| {
            io::Error::other(format!(
                "BIF file name too long: {} bytes",
                entry.file_name.len()
            ))
        })?;
        cursor = cursor
            .checked_add(len + 1)
            .ok_or_else(|| io::Error::other("KEY file size overflow"))?;
    }
    let resources_offset = cursor;

    let total = (resources_offset + n_resources * RESOURCE_ENTRY_SIZE) as usize;
    let mut buf = Vec::with_capacity(total);

    // 1. Header (24 bytes). The signature/version slots are 4 bytes each,
    //    space-padded to match what the importer trims off.
    write_fixed_string(&mut buf, &key.signature, 4)?;
    write_fixed_string(&mut buf, &key.version, 4)?;
    buf.extend_from_slice(&n_bifs.to_le_bytes());
    buf.extend_from_slice(&n_resources.to_le_bytes());
    buf.extend_from_slice(&bif_offset.to_le_bytes());
    buf.extend_from_slice(&resources_offset.to_le_bytes());

    // 2. BIF entries.
    for (entry, &string_offset) in key.bif_entries.iter().zip(string_offsets.iter()) {
        if !is_demo {
            buf.extend_from_slice(&entry.file_size.unwrap_or(0).to_le_bytes());
        }
        buf.extend_from_slice(&string_offset.to_le_bytes());
        let string_length = u16::try_from(entry.file_name.len() + 1).map_err(|_| {
            io::Error::other(format!(
                "BIF file name too long: {} bytes",
                entry.file_name.len()
            ))
        })?;
        buf.extend_from_slice(&string_length.to_le_bytes());
        buf.extend_from_slice(&entry.directory.to_u16().to_le_bytes());
    }

    // 3. BIF filename strings (null-terminated, in the same order as the entries).
    for entry in &key.bif_entries {
        buf.extend_from_slice(entry.file_name.as_bytes());
        buf.push(0);
    }

    // 4. Resource entries.
    for entry in &key.resource_entries {
        write_resource_entry(&mut buf, entry)?;
    }

    debug_assert_eq!(buf.len(), total);
    Ok(buf)
}

/// Returns `Some(true)` if every BIF entry is demo-style, `Some(false)` if
/// every entry is standard-style, `None` if there are no entries. Mixed
/// inputs return an error.
fn detect_demo(key: &Key) -> io::Result<Option<bool>> {
    let mut iter = key.bif_entries.iter();
    let Some(first) = iter.next() else {
        return Ok(None);
    };
    let first_is_demo = first.file_size.is_none();
    for entry in iter {
        if entry.file_size.is_none() != first_is_demo {
            return Err(io::Error::other(
                "cannot export KEY: BIF entries mix demo (file_size=None) and standard (file_size=Some) variants",
            ));
        }
    }
    Ok(Some(first_is_demo))
}

fn write_resource_entry(buf: &mut Vec<u8>, entry: &ResourceEntry) -> io::Result<()> {
    write_fixed_string(buf, &entry.resource_name, 8)?;
    buf.extend_from_slice(&entry.r#type.to_u16().to_le_bytes());
    let bif_entries_index = u32::try_from(entry.bif_entries_index).map_err(|_| {
        io::Error::other(format!(
            "bif_entries_index out of range: {}",
            entry.bif_entries_index
        ))
    })?;
    if bif_entries_index > 0xFFF {
        return Err(io::Error::other(format!(
            "bif_entries_index exceeds 12 bits: {bif_entries_index}"
        )));
    }
    if entry.bif_resource_locator > 0x000F_FFFF {
        return Err(io::Error::other(format!(
            "bif_resource_locator exceeds 20 bits: {:#x}",
            entry.bif_resource_locator
        )));
    }
    let locator = (bif_entries_index << 20) | entry.bif_resource_locator;
    buf.extend_from_slice(&locator.to_le_bytes());
    Ok(())
}

fn write_fixed_string(buf: &mut Vec<u8>, s: &str, size: usize) -> io::Result<()> {
    let bytes = s.as_bytes();
    if bytes.len() > size {
        return Err(io::Error::other(format!(
            "field {s:?} too long ({} bytes) for {}-byte slot",
            bytes.len(),
            size
        )));
    }
    buf.extend_from_slice(bytes);
    buf.resize(buf.len() + (size - bytes.len()), 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KeyImporter;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_test_utils::{constants::ALL_RESOURCES_DIRS, get_assets_path};

    #[test]
    fn test_export_roundtrip_all_chitin_files() {
        for (dir, _game) in ALL_RESOURCES_DIRS {
            let dir = get_assets_path().join("KEY").join(dir);
            let key_path = CaseInsensitiveFS::new(&dir)
                .unwrap()
                .get_path("/CHITIN.KEY")
                .unwrap();
            let original = KeyImporter { name: "key_rt" }
                .import(&DataSource::new(key_path.path()))
                .unwrap();

            let mut buf: Vec<u8> = Vec::new();
            KeyExporter.export(&original, &mut buf).unwrap();
            let re_imported = KeyImporter { name: "key_rt2" }
                .import(&DataSource::new(buf))
                .unwrap();

            assert_eq!(
                re_imported,
                original,
                "round-trip mismatch for {}",
                dir.display()
            );
        }
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let dir = get_assets_path().join("KEY").join("bg");
        let key_path = CaseInsensitiveFS::new(&dir)
            .unwrap()
            .get_path("/CHITIN.KEY")
            .unwrap();
        let original = KeyImporter { name: "key_rt" }
            .import(&DataSource::new(key_path.path()))
            .unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        KeyExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = KeyImporter { name: "key_rt2" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();

        assert_eq!(re_imported, original);
    }

    #[test]
    fn test_export_empty_key() {
        let key = Key {
            signature: "KEY".into(),
            version: "V1".into(),
            bif_entries: Vec::new(),
            resource_entries: Vec::new(),
        };
        let mut buf: Vec<u8> = Vec::new();
        KeyExporter.export(&key, &mut buf).unwrap();
        let rt = KeyImporter { name: "key_empty" }
            .import(&DataSource::new(buf))
            .unwrap();
        assert_eq!(rt, key);
    }

    #[test]
    fn test_export_rejects_mixed_demo_and_standard() {
        use crate::{BifDirectory, BifEntry};
        let key = Key {
            signature: "KEY".into(),
            version: "V1".into(),
            bif_entries: vec![
                BifEntry {
                    index: 0,
                    file_name: "data/a.bif".into(),
                    file_size: Some(42),
                    directory: BifDirectory::Root,
                },
                BifEntry {
                    index: 1,
                    file_name: "data/b.bif".into(),
                    file_size: None,
                    directory: BifDirectory::Root,
                },
            ],
            resource_entries: Vec::new(),
        };
        let mut buf: Vec<u8> = Vec::new();
        let err = KeyExporter.export(&key, &mut buf).unwrap_err();
        assert!(err.to_string().contains("mix demo"));
    }
}
