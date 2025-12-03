use std::{fs::File, io::{self, BufReader}, path::Path};

use encoding_rs::WINDOWS_1252;

use crate::io::Reader;

#[derive(Debug, PartialEq, Eq)]
pub struct Key {
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
    pub offset: u32,
    pub size: u32,
}

impl Key {
    fn read_key_file(file_path: &Path) -> Result<Key, io::Error> {
        let mut reader = Reader::with_file(file_path, WINDOWS_1252)?;
        let signature = reader.read_string::<4>()?.trim().to_string();
        let version = reader.read_string::<4>()?.trim().to_string();

        if !(signature.eq("KEY") && version.eq("V1")) {
            return Err(io::Error::new(io::ErrorKind::Other, "Wrong file type"));
        }

        let bif_size = reader.read_u32()?;
        let resources_size = reader.read_u32()?;
        let bif_offset = reader.read_u32()?;
        let resources_offset = reader.read_u32()?;

        // checking for BG1 Demo variant of KEY file format
        {
            let is_demo = reader.read_u32_at(bif_offset as u64)? - bif_offset == bif_size * 0x8
                && reader.read_u32_at(bif_offset as u64 + 4)? - bif_offset != bif_size * 0xc;
            let biff_entry_size = if is_demo { 0x8 } else { 0xc };
        }

        let mut biff_entryies = Vec::new();
        for _ in 0..bif_size {
//            biff_entryies.push(BiffEntry { offset, size });
        }



        Ok(Key {
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
    
    fn read_biff_entry(reader: &mut Reader<BufReader<File>>, offset: u64, index: u64, is_demo: bool) -> std::io::Result<BiffEntry> {

    if !is_demo {
      let file_size = reader.read_u32_at(offset)?;
    }

    let string_offset = reader.read_u32()?;
    let string_length = reader.read_u16()?;
    let location = reader.read_u16()? & 0xffff;

    let offset_position= reader.position()?;

    // let file_name = reader.read_string_at(string_offset as u64, string_length as usize)?;
    //                  StreamUtils.readString(buffer, this.stringOffset, stringLength - 1);


    reader.set_position(offset_position)?;

    todo!("Implement BiffEntry");

    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_key_file() {
        let key = Key::read_key_file(&Path::new(
            "/home/ufo/Temp/Games/Baldur's Gate - Enhanced Edition/chitin.key",
        ))
        .unwrap();
        assert_eq!(
            key,
            Key {
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
