use std::io::{self, BufRead, Seek};

use infinitier_common::ResourceType;
use infinitier_datasource::{DataSource, Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{BifDirectory, BifEntry, Key, ResourceEntry};

/// A KEY file importer
pub struct KeyImporter<'a> {
    pub name: &'a str,
}

impl Importer for KeyImporter<'_> {
    type T = Key;

    fn import(&self, data: &DataSource) -> std::io::Result<Key> {
        debug!("Importing {} [KEY] from datasource {:?}", self.name, data);

        let mut reader = data.reader()?;
        let signature = reader.read_string(4)?.trim().to_string();
        let version = reader.read_string(4)?.trim().to_string();

        if !(signature.eq("KEY") && version.eq("V1")) {
            error!(
                "Not a KEY V1 file ({}): signature={:?} version={:?}",
                self.name, signature, version
            );
            return Err(io::Error::other("Wrong file type"));
        }

        let bif_size = reader.read_u32()? as u64;
        let resources_size = reader.read_u32()? as u64;
        let bif_offset = reader.read_u32()? as u64;
        let resources_offset = reader.read_u32()? as u64;

        // checking for BG1 Demo variant of KEY file format. The demo
        // heuristic inspects bytes at `bif_offset` so it requires at least
        // one BIF entry to be present — for an empty key (0 BIF entries)
        // the layout is unambiguous and the demo flag doesn't matter.
        let is_demo = bif_size > 0
            && reader.read_u32_at(bif_offset)? as u64 - bif_offset == bif_size * 0x8
            && reader.read_u32_at(bif_offset + 4)? as u64 - bif_offset != bif_size * 0xc;

        // reading BIF entries
        let mut bif_entries = Vec::new();
        reader.set_position(bif_offset)?;
        for i in 0..bif_size {
            bif_entries.push(read_bif_entry(&mut reader, i, is_demo)?);
        }

        // reading resource entries
        let mut resource_entries = Vec::new();
        reader.set_position(resources_offset)?;
        for _ in 0..resources_size {
            resource_entries.push(read_resource_entry(&mut reader, resource_entries.last())?);
        }

        debug!(
            "Loaded {} [KEY]: {} bif entries, {} resource entries",
            self.name,
            bif_entries.len(),
            resource_entries.len()
        );
        Ok(Key {
            signature,
            version,
            bif_entries,
            resource_entries,
        })
    }
}

/// Reads a BIF entry inside a KEY file.
fn read_bif_entry<R: BufRead + Seek>(
    reader: &mut Reader<R>,
    index: u64,
    is_demo: bool,
) -> std::io::Result<BifEntry> {
    let file_size = if !is_demo {
        Some(reader.read_u32()?)
    } else {
        None
    };

    let string_offset = reader.read_u32()?;
    let string_length = reader.read_u16()?;
    let location = reader.read_u16()?;

    let offset_position = reader.position()?;

    let mut file_name = reader
        .read_string_at(string_offset as u64, string_length as u64 - 1)?
        .trim()
        .to_lowercase()
        .replace("\\", "/")
        .replace(":", "/");

    if file_name.starts_with("/") {
        file_name = file_name[1..].to_string();
    }

    reader.set_position(offset_position)?;

    debug!("BIF entry: {} - {}", index, file_name);
    Ok(BifEntry {
        file_size,
        index,
        file_name,
        directory: BifDirectory::from(location),
    })
}

/// Reads a Resource entry inside a KEY file.
fn read_resource_entry<R: BufRead + Seek>(
    reader: &mut Reader<R>,
    previous_entry: Option<&ResourceEntry>,
) -> std::io::Result<ResourceEntry> {
    let resource_name = reader.read_string(8)?.trim().to_lowercase();
    let resource_type = reader.read_u16()?;
    let locator = reader.read_u32()?;

    let bif_entries_index = ((locator >> 20) & 0xfff) as u64;

    let mut index_inside_bif_file = 0;
    if let Some(previous_entry) = previous_entry
        && bif_entries_index == previous_entry.bif_entries_index
    {
        index_inside_bif_file = previous_entry.index_into_bif_file + 1;
    };

    Ok(ResourceEntry {
        resource_name,
        r#type: ResourceType::from(resource_type),
        bif_entries_index,
        index_into_bif_file: index_inside_bif_file,
        bif_resource_locator: locator & 0x000FFFFF,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_test_utils::{constants::ALL_RESOURCES_DIRS, get_assets_path, parse_json_file};

    #[test]
    fn test_read_key_file() {
        for (dir, _game) in ALL_RESOURCES_DIRS {
            let dir = get_assets_path().join("KEY").join(dir);
            let key_path = CaseInsensitiveFS::new(&dir)
                .unwrap()
                .get_path("/CHITIN.KEY")
                .unwrap();
            let json_path = key_path.path().parent().unwrap().join("chitin.json");

            let expected: Key = parse_json_file(&json_path);

            let actual = KeyImporter { name: "key_test" }
                .import(&DataSource::new(key_path.path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", key_path.path().display()));

            assert_eq!(actual, expected, "key mismatch for {}", dir.display());
        }
    }
}
