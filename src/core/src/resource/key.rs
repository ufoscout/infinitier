use std::{fs::File, io::{self, BufReader}, path::{Path, PathBuf}};

use encoding_rs::WINDOWS_1252;

use crate::io::Reader;

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub struct BiffEntry {
    pub index: u64,
    pub file_name: String,
    pub file: PathBuf,
    pub directory: BiffDirectory,
}

#[derive(Debug, PartialEq, Eq)]
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
}

impl BiffDirectory {
    fn from(bit: u16) -> std::io::Result<Self> {
        match bit {
            0 => Ok(BiffDirectory::Root),
            1 => Ok(BiffDirectory::Cache),
            2 => Ok(BiffDirectory::Cd1),
            3 => Ok(BiffDirectory::Cd2),
            4 => Ok(BiffDirectory::Cd3),
            5 => Ok(BiffDirectory::Cd4),
            6 => Ok(BiffDirectory::Cd5),
            7 => Ok(BiffDirectory::Cd6),
            8 => Ok(BiffDirectory::Cd7),
            i => Err(io::Error::new(io::ErrorKind::Other, format!("Unknown directory bit: {}", i))),
        }
    }
}

impl Key {
    fn read_key_file(file_path: &Path) -> Result<Key, io::Error> {
        let mut reader = Reader::with_file(file_path, WINDOWS_1252)?;
        let signature = reader.read_string(4)?.trim().to_string();
        let version = reader.read_string(4)?.trim().to_string();

        if !(signature.eq("KEY") && version.eq("V1")) {
            return Err(io::Error::new(io::ErrorKind::Other, "Wrong file type"));
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
           biff_entryies.push(BiffEntry::read_biff_entry(&mut reader, file_path, i, is_demo)?);
        }



        Ok(Key {
            file: file_path.to_path_buf(),
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
    
    fn read_biff_entry(reader: &mut Reader<BufReader<File>>, key_file: &Path, index: u64, is_demo: bool) -> std::io::Result<BiffEntry> {

    if !is_demo {
      let file_size = reader.read_u32()?;
    }

    let string_offset = reader.read_u32()?;
    let string_length = reader.read_u16()?;
    let location = reader.read_u16()? & 0xffff;

    let offset_position= reader.position()?;

    let file_name = reader.read_string_at(string_offset as u64, string_length as u64 -1)?.to_lowercase().replace("\\", "/");
    //                  StreamUtils.readString(buffer, this.stringOffset, stringLength - 1);


    reader.set_position(offset_position)?;

    Ok(BiffEntry { 
        file: find_biff_file(key_file, &file_name)?,
        index,
        file_name,
        directory: BiffDirectory::from(location)?
     })

    }

}

fn find_biff_file(key_file: &Path, file_name: &str) -> std::io::Result<PathBuf> {
    let parent = key_file.parent().expect("Key file has no parent directory");

    let paths = vec![
        parent.to_path_buf(),
        parent.join("data"),
        parent.join("cache"),
        parent.join("cd1"),
        parent.join("cd2"),
        parent.join("cd3"),
        parent.join("cd4"),
        parent.join("cd5"),
        parent.join("cd6"),
        parent.join("cd7"),
    ];

    for path in paths {
        let file_path = path.join(file_name);
        if file_path.is_file() {
            return Ok(file_path);
        }
    }

    Err(io::Error::new(io::ErrorKind::NotFound, format!("File not found: {}", file_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_key_file() {
        let key = Key::read_key_file(&Path::new(
            // "/home/ufo/Temp/Games/Baldur's Gate 2 - Enhanced Edition/chitin.key",
            "/home/ufo/Temp/Games/Baldur's Gate 2/CHITIN.KEY",
        ))
        .unwrap();
        assert_eq!(
            key,
            Key {
                file: PathBuf::from("/home/ufo/Temp/Games/Baldur's Gate - Enhanced Edition/chitin.key"),
                signature: "KEY".to_string(),
                version: "V1".to_string(),
                resources_offset: 2376,
                resources_size: 37341,
                bif_offset: 24,
                bif_size: 82,
                biff_entryies: vec![],
            }
        );
    }
}
