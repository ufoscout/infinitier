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
    pub biff_entryies: Vec<BiffEntry>,
}

/// A BIFF entry inside a KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiffEntry {
    pub index: u64,
    pub file_name: String,
    pub file_size: Option<u32>,
    pub file: Option<PathBuf>,
    pub directory: BiffDirectory,
}

/// Baldur's Gate 2 BIFF directory where a file "could" be found
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiffDirectory {
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

impl BiffDirectory {
    fn from(bit: u16) -> Self {
        match bit {
            0 => BiffDirectory::Root,
            1 => BiffDirectory::Cache,
            2 => BiffDirectory::Cd1,
            3 => BiffDirectory::Cd2,
            4 => BiffDirectory::Cd3,
            5 => BiffDirectory::Cd4,
            6 => BiffDirectory::Cd5,
            7 => BiffDirectory::Cd6,
            8 => BiffDirectory::Cd7,
            i => BiffDirectory::Unknown(i),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            BiffDirectory::Root => 0,
            BiffDirectory::Cache => 1,
            BiffDirectory::Cd1 => 2,
            BiffDirectory::Cd2 => 3,
            BiffDirectory::Cd3 => 4,
            BiffDirectory::Cd4 => 5,
            BiffDirectory::Cd5 => 6,
            BiffDirectory::Cd6 => 7,
            BiffDirectory::Cd7 => 8,
            BiffDirectory::Unknown(i) => *i,
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
        let mut biff_entryies = Vec::new();
        for i in 0..bif_size as u64 {
            biff_entryies.push(BiffEntry::read_biff_entry(fs, &mut reader, i, is_demo)?);
        }

        Ok(Key {
            file: key_file_path.to_path_buf(),
            signature,
            version,
            resources_offset,
            resources_size,
            bif_offset,
            bif_size,
            biff_entryies,
        })
    }
}

impl BiffEntry {

    /// Reads a BIFF entry inside a KEY file
    fn read_biff_entry(
        fs: &CaseInsensitiveFS,
        reader: &mut Reader<BufReader<File>>,
        index: u64,
        is_demo: bool,
    ) -> std::io::Result<BiffEntry> {

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

        Ok(BiffEntry {
            file: find_biff_file(fs, &file_name),
            file_size,
            index,
            file_name,
            directory: BiffDirectory::from(location),
        })
    }
}

fn find_biff_file(fs: &CaseInsensitiveFS, file_name: &str) -> Option<PathBuf> {
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
        assert_eq!(BiffDirectory::from(0), BiffDirectory::Root);
        assert_eq!(BiffDirectory::from(1), BiffDirectory::Cache);
        assert_eq!(BiffDirectory::from(2), BiffDirectory::Cd1);
        assert_eq!(BiffDirectory::from(3), BiffDirectory::Cd2);
        assert_eq!(BiffDirectory::from(4), BiffDirectory::Cd3);
        assert_eq!(BiffDirectory::from(5), BiffDirectory::Cd4);
        assert_eq!(BiffDirectory::from(6), BiffDirectory::Cd5);
        assert_eq!(BiffDirectory::from(7), BiffDirectory::Cd6);
        assert_eq!(BiffDirectory::from(8), BiffDirectory::Cd7);
        assert_eq!(BiffDirectory::from(9), BiffDirectory::Unknown(9));

        for i in 0..256 {
            assert_eq!(BiffDirectory::from(i).to_u16(), i);
        }
    }

    #[test]
    fn test_read_key_file() {

        for i in ALL_RESOURCES_DIRS {
            let fs = CaseInsensitiveFS::new(i).unwrap();
            let key = Key::import(&fs, "/CHITIN.KEY").unwrap();

            assert_eq!(key.file, fs.get_path("CHITIN.KEY").unwrap());
            assert_eq!(key.bif_size, key.biff_entryies.len() as u32);
            assert_json_snapshot!(format!("key_file_{i}"), key, {
                ".file" => ""
            });
        }

    }

}
