use std::io::{self, BufRead, Seek};

use serde::{Deserialize, Serialize};

use infinitier_datasource::{DataSource, Importer, Reader};
use log::{debug, error};

pub use infinitier_common::ResourceType;

/// A KEY file importer
pub struct KeyImporter<'a> {
    pub name: &'a str,
}

impl<'a> Importer for KeyImporter<'a> {
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

        // checking for BG1 Demo variant of KEY file format
        let is_demo = reader.read_u32_at(bif_offset)? as u64 - bif_offset == bif_size * 0x8
            && reader.read_u32_at(bif_offset + 4)? as u64 - bif_offset != bif_size * 0xc;

        // reading BIF entries
        let mut bif_entries = Vec::new();
        reader.set_position(bif_offset)?;
        for i in 0..bif_size {
            bif_entries.push(BifEntry::read_entry(&mut reader, i, is_demo)?);
        }

        // reading resource entries
        let mut resource_entries = Vec::new();
        reader.set_position(resources_offset)?;
        for _ in 0..resources_size {
            resource_entries.push(ResourceEntry::read_entry(
                &mut reader,
                resource_entries.last(),
            )?);
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
            resources_offset,
            bif_offset,
            bif_entries,
            resource_entries,
        })
    }
}

/// A KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub signature: String,
    pub version: String,
    pub resources_offset: u64,
    pub bif_offset: u64,
    pub bif_entries: Vec<BifEntry>,
    pub resource_entries: Vec<ResourceEntry>,
}

/// A BIFF entry inside a KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BifEntry {
    pub index: u64,
    pub file_name: String,
    pub file_size: Option<u32>,
    pub directory: BifDirectory,
}

/// Baldur's Gate 2 BIFF directory where a file "could" be found
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BifDirectory {
    Root,
    Cache,
    Cd1,
    Cd2,
    Cd3,
    Cd4,
    Cd5,
    Cd6,
    Cd7,
    Unknown(u16),
}

impl BifDirectory {
    fn from(bit: u16) -> Self {
        match bit {
            0 => BifDirectory::Root,
            1 => BifDirectory::Cache,
            2 => BifDirectory::Cd1,
            3 => BifDirectory::Cd2,
            4 => BifDirectory::Cd3,
            5 => BifDirectory::Cd4,
            6 => BifDirectory::Cd5,
            7 => BifDirectory::Cd6,
            8 => BifDirectory::Cd7,
            i => BifDirectory::Unknown(i),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            BifDirectory::Root => 0,
            BifDirectory::Cache => 1,
            BifDirectory::Cd1 => 2,
            BifDirectory::Cd2 => 3,
            BifDirectory::Cd3 => 4,
            BifDirectory::Cd4 => 5,
            BifDirectory::Cd5 => 6,
            BifDirectory::Cd6 => 7,
            BifDirectory::Cd7 => 8,
            BifDirectory::Unknown(i) => *i,
        }
    }
}

/// A resource entry inside a KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct ResourceEntry {
    /// Resource name without extension.
    pub resource_name: String,
    /// Resource type.
    pub r#type: ResourceType,
    /// Index of the entry in the key.bif_entries vector that contains this resource
    pub bif_entries_index: u64,
    /// Index of this resource into the bif.entries vector (sequential counter, legacy)
    pub index_into_bif_file: u64,
    /// Lower 20 bits of the KEY locator for this resource.
    /// Used to find the matching entry inside the BIF by locator-bit comparison.
    pub bif_resource_locator: u32,
}

impl BifEntry {
    /// Reads a BIF entry inside a KEY file
    fn read_entry<R: BufRead + Seek>(
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
}

impl ResourceEntry {
    /// Reads a Resource entry inside a KEY file
    fn read_entry<R: BufRead + Seek>(
        reader: &mut Reader<R>,
        previous_entry: Option<&Self>,
    ) -> std::io::Result<ResourceEntry> {
        let resource_name = reader.read_string(8)?.trim().to_string();
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
}

#[cfg(test)]
mod tests {
    use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};
    use infinitier_test_utils::{constants::ALL_RESOURCES_DIRS, get_assets_path, parse_json_file};

    use super::*;

    #[test]
    fn test_biff_directory() {
        assert_eq!(BifDirectory::from(0), BifDirectory::Root);
        assert_eq!(BifDirectory::from(1), BifDirectory::Cache);
        assert_eq!(BifDirectory::from(2), BifDirectory::Cd1);
        assert_eq!(BifDirectory::from(3), BifDirectory::Cd2);
        assert_eq!(BifDirectory::from(4), BifDirectory::Cd3);
        assert_eq!(BifDirectory::from(5), BifDirectory::Cd4);
        assert_eq!(BifDirectory::from(6), BifDirectory::Cd5);
        assert_eq!(BifDirectory::from(7), BifDirectory::Cd6);
        assert_eq!(BifDirectory::from(8), BifDirectory::Cd7);
        assert_eq!(BifDirectory::from(9), BifDirectory::Unknown(9));

        for i in 0..256 {
            assert_eq!(BifDirectory::from(i).to_u16(), i);
        }
    }

    #[test]
    fn test_read_key_file() {
        for (dir, _game) in ALL_RESOURCES_DIRS {
            let dir = get_assets_path().join("KEY").join(dir);
            let key_path = CaseInsensitiveFS::new(&dir)
                .unwrap()
                .get_path(&CaseInsensitivePath::new("/CHITIN.KEY"))
                .unwrap();
            let json_path = key_path.parent().unwrap().join("chitin.json");

            let expected: Key = parse_json_file(&json_path);

            let actual = KeyImporter { name: "key_test" }
                .import(&DataSource::new(key_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", key_path.display()));

            assert_eq!(actual, expected, "key mismatch for {}", dir.display());
        }
    }
}
