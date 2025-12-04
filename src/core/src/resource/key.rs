use std::{
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
};

use encoding_rs::WINDOWS_1252;
use serde::{Deserialize, Serialize};

use crate::{constants::FILE_FOLDERS, fs::CaseInsensitiveFS, io::Reader};

/// A KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub file: PathBuf,
    pub signature: String,
    pub version: String,
    pub resources_offset: u32,
    pub resources_size: u32,
    pub bif_offset: u32,
    pub bif_size: u32,
    pub bif_entryies: Vec<BifEntry>,
}

/// A BIFF entry inside a KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BifEntry {
    pub index: u64,
    pub file_name: String,
    pub file_size: Option<u32>,
    pub file: Option<PathBuf>,
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

impl Key {

    /// Reads a KEY file
    pub fn import(fs: &CaseInsensitiveFS, file_name: &str) -> Result<Key, io::Error> {
        let key_file_path = fs.get_path(file_name)?;
        let mut reader = Reader::with_file(&key_file_path, WINDOWS_1252)?;
        let signature = reader.read_string(4)?.trim().to_string();
        let version = reader.read_string(4)?.trim().to_string();

        if !(signature.eq("KEY") && version.eq("V1")) {
            return Err(io::Error::other("Wrong file type"));
        }

        let bif_size = reader.read_u32()?;
        let resources_size = reader.read_u32()?;
        let bif_offset = reader.read_u32()?;
        let resources_offset = reader.read_u32()?;

        let offset_position = reader.position()?;

        // checking for BG1 Demo variant of KEY file format
        let is_demo = reader.read_u32_at(bif_offset as u64)? - bif_offset == bif_size * 0x8
            && reader.read_u32_at(bif_offset as u64 + 4)? - bif_offset != bif_size * 0xc;

        reader.set_position(offset_position)?;
        let mut bif_entryies = Vec::new();
        for i in 0..bif_size as u64 {
            bif_entryies.push(BifEntry::read_bif_entry(fs, &mut reader, i, is_demo)?);
        }

        Ok(Key {
            file: key_file_path.to_path_buf(),
            signature,
            version,
            resources_offset,
            resources_size,
            bif_offset,
            bif_size,
            bif_entryies,
        })
    }
}

impl BifEntry {

    /// Reads a BIF entry inside a KEY file
    fn read_bif_entry(
        fs: &CaseInsensitiveFS,
        reader: &mut Reader<BufReader<File>>,
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

        let bif_file = find_bif_file(fs, &file_name)
        .or_else(|| find_bif_file(fs, &file_name.replace(".bif", ".cbf"))); 

        Ok(BifEntry {
            file: bif_file,
            file_size,
            index,
            file_name,
            directory: BifDirectory::from(location),
        })
    }
}

fn find_bif_file(fs: &CaseInsensitiveFS, file_name: &str) -> Option<PathBuf> {
    for path in FILE_FOLDERS {
        let search_name = format!("{}{}", path, file_name);
        if let Some(path) = fs.get_path_opt(&search_name)
            && path.is_file() {
                return Some(path);
            }
    }
    None
}

#[cfg(test)]
mod tests {
    use insta::assert_json_snapshot;

    use crate::test_utils::ALL_RESOURCES_DIRS;

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

        for i in ALL_RESOURCES_DIRS {
            let fs = CaseInsensitiveFS::new(i).unwrap();
            let key = Key::import(&fs, "/CHITIN.KEY").unwrap();

            assert_eq!(key.file, fs.get_path("CHITIN.KEY").unwrap());
            assert_eq!(key.bif_size, key.bif_entryies.len() as u32);
            assert_json_snapshot!(format!("key_file_{i}"), key, {
                ".file" => ""
            });
        }

    }

}
