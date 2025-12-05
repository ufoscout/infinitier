use std::{
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
};

use encoding_rs::WINDOWS_1252;
use serde::{Deserialize, Serialize};

use crate::{
    constants::FILE_FOLDERS,
    fs::CaseInsensitiveFS,
    io::{Importer, Reader},
};

/// A KEY file importer
pub struct KeyImporter {
    fs: CaseInsensitiveFS,
    file_name: String,
}

impl KeyImporter {
    /// Creates a new KEY file importer
    pub fn new(fs: CaseInsensitiveFS, file_name: String) -> KeyImporter {
        KeyImporter { fs, file_name }
    }
}

impl Importer for KeyImporter {
    type T = Key;

    fn import(&self) -> std::io::Result<Key> {
        Key::import(&self.fs, &self.file_name)
    }
}

/// A KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub file: PathBuf,
    pub signature: String,
    pub version: String,
    pub resources_offset: u32,
    pub bif_offset: u32,
    pub bif_entries: Vec<BifEntry>,
    pub resource_entries: Vec<ResourceEntry>,
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

/// A resource entry inside a KEY file
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceEntry {
    /// Resource name without extension.
    pub resource_name: String,
    /// Resource type.
    pub r#type: ResourceType,

    pub locator: u32,
}

impl Key {
    /// Reads a KEY file
    fn import(fs: &CaseInsensitiveFS, file_name: &str) -> Result<Key, io::Error> {
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

        // checking for BG1 Demo variant of KEY file format
        let is_demo = reader.read_u32_at(bif_offset as u64)? - bif_offset == bif_size * 0x8
            && reader.read_u32_at(bif_offset as u64 + 4)? - bif_offset != bif_size * 0xc;

        // reading BIF entries
        let mut bif_entries = Vec::new();
        reader.set_position(bif_offset as u64)?;
        for i in 0..bif_size as u64 {
            bif_entries.push(BifEntry::read_entry(fs, &mut reader, i, is_demo)?);
        }

        // reading resource entries
        let mut resource_entries = Vec::new();
        reader.set_position(resources_offset as u64)?;
        for _ in 0..resources_size as u64 {
            resource_entries.push(ResourceEntry::read_entry(&mut reader)?);
        }

        Ok(Key {
            file: key_file_path.to_path_buf(),
            signature,
            version,
            resources_offset,
            bif_offset,
            bif_entries,
            resource_entries,
        })
    }
}

impl BifEntry {
    /// Reads a BIF entry inside a KEY file
    fn read_entry(
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
            && path.is_file()
        {
            return Some(path);
        }
    }
    None
}

impl ResourceEntry {
    /// Reads a Resource entry inside a KEY file
    fn read_entry(reader: &mut Reader<BufReader<File>>) -> std::io::Result<ResourceEntry> {
        let resource_name = reader.read_string(8)?.trim().to_string();
        let resource_type = reader.read_u16()?;
        let locator = reader.read_u32()?;

        Ok(ResourceEntry {
            resource_name,
            r#type: ResourceType::from(resource_type),
            locator,
        })
    }
}

/// A Resource file type
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    Bmp,
    Mve,
    Wav,
    Wfx,
    Plt,
    Tga,
    Bam,
    Wed,
    Chu,
    Tis,
    Mos,
    Itm,
    Spl,
    Bcs,
    Ids,
    Cre,
    Are,
    Dlg,
    Two,
    Gam,
    Sto,
    Wmp,
    Eff,
    Bs,
    Chr,
    Vvc,
    Vef,
    Pro,
    Bio,
    Wbm,
    Fnt,
    Gui,
    Sql,
    Pvrz,
    Glsl,
    Tot,
    Toh,
    Menu,
    Lua,
    Ttf,
    Png,
    Bah,
    Ini,
    Src,
    Maze,
    Mus,
    Acm,
    Unknown(u16),
}

impl ResourceType {
    /// Returns the `ResourceType` enum variant based on the given hexadecimal value.
    pub fn from(bit: u16) -> Self {
        match bit {
            0x001 => ResourceType::Bmp,
            0x002 => ResourceType::Mve,
            0x004 => ResourceType::Wav,
            0x005 => ResourceType::Wfx,
            0x006 => ResourceType::Plt,
            0x3b8 => ResourceType::Tga,
            0x3e8 => ResourceType::Bam,
            0x3e9 => ResourceType::Wed,
            0x3ea => ResourceType::Chu,
            0x3eb => ResourceType::Tis,
            0x3ec => ResourceType::Mos,
            0x3ed => ResourceType::Itm,
            0x3ee => ResourceType::Spl,
            0x3ef => ResourceType::Bcs,
            0x3f0 => ResourceType::Ids,
            0x3f1 => ResourceType::Cre,
            0x3f2 => ResourceType::Are,
            0x3f3 => ResourceType::Dlg,
            0x3f4 => ResourceType::Two,
            0x3f5 => ResourceType::Gam,
            0x3f6 => ResourceType::Sto,
            0x3f7 => ResourceType::Wmp,
            0x3f8 => ResourceType::Eff,
            0x3f9 => ResourceType::Bs,
            0x3fa => ResourceType::Chr,
            0x3fb => ResourceType::Vvc,
            0x3fc => ResourceType::Vef,
            0x3fd => ResourceType::Pro,
            0x3fe => ResourceType::Bio,
            0x3ff => ResourceType::Wbm,
            0x400 => ResourceType::Fnt,
            0x402 => ResourceType::Gui,
            0x403 => ResourceType::Sql,
            0x404 => ResourceType::Pvrz,
            0x405 => ResourceType::Glsl,
            0x406 => ResourceType::Tot,
            0x407 => ResourceType::Toh,
            0x408 => ResourceType::Menu,
            0x409 => ResourceType::Lua,
            0x40a => ResourceType::Ttf,
            0x40b => ResourceType::Png,
            0x44c => ResourceType::Bah,
            0x802 => ResourceType::Ini,
            0x803 => ResourceType::Src,
            0x804 => ResourceType::Maze,
            0xffe => ResourceType::Mus,
            0xfff => ResourceType::Acm,
            i => ResourceType::Unknown(i),
        }
    }

    /// Returns the hexadecimal value of the `ResourceType` enum variant.
    pub fn to_u16(&self) -> u16 {
        match self {
            ResourceType::Bmp => 0x001,
            ResourceType::Mve => 0x002,
            ResourceType::Wav => 0x004,
            ResourceType::Wfx => 0x005,
            ResourceType::Plt => 0x006,
            ResourceType::Tga => 0x3b8,
            ResourceType::Bam => 0x3e8,
            ResourceType::Wed => 0x3e9,
            ResourceType::Chu => 0x3ea,
            ResourceType::Tis => 0x3eb,
            ResourceType::Mos => 0x3ec,
            ResourceType::Itm => 0x3ed,
            ResourceType::Spl => 0x3ee,
            ResourceType::Bcs => 0x3ef,
            ResourceType::Ids => 0x3f0,
            ResourceType::Cre => 0x3f1,
            ResourceType::Are => 0x3f2,
            ResourceType::Dlg => 0x3f3,
            ResourceType::Two => 0x3f4,
            ResourceType::Gam => 0x3f5,
            ResourceType::Sto => 0x3f6,
            ResourceType::Wmp => 0x3f7,
            ResourceType::Eff => 0x3f8,
            ResourceType::Bs => 0x3f9,
            ResourceType::Chr => 0x3fa,
            ResourceType::Vvc => 0x3fb,
            ResourceType::Vef => 0x3fc,
            ResourceType::Pro => 0x3fd,
            ResourceType::Bio => 0x3fe,
            ResourceType::Wbm => 0x3ff,
            ResourceType::Fnt => 0x400,
            ResourceType::Gui => 0x402,
            ResourceType::Sql => 0x403,
            ResourceType::Pvrz => 0x404,
            ResourceType::Glsl => 0x405,
            ResourceType::Tot => 0x406,
            ResourceType::Toh => 0x407,
            ResourceType::Menu => 0x408,
            ResourceType::Lua => 0x409,
            ResourceType::Ttf => 0x40a,
            ResourceType::Png => 0x40b,
            ResourceType::Bah => 0x44c,
            ResourceType::Ini => 0x802,
            ResourceType::Src => 0x803,
            ResourceType::Maze => 0x804,
            ResourceType::Mus => 0xffe,
            ResourceType::Acm => 0xfff,
            ResourceType::Unknown(i) => *i,
        }
    }

    /// Returns the extension of the `ResourceType` enum variant as a string, or `None` if it is unknown.
    pub fn get_extension(&self) -> Option<&str> {
        match self {
            ResourceType::Bmp => Some("bmp"),
            ResourceType::Mve => Some("mve"),
            ResourceType::Wav => Some("wav"),
            ResourceType::Wfx => Some("wfx"),
            ResourceType::Plt => Some("plt"),
            ResourceType::Tga => Some("tga"),
            ResourceType::Bam => Some("bam"),
            ResourceType::Wed => Some("wed"),
            ResourceType::Chu => Some("chu"),
            ResourceType::Tis => Some("tis"),
            ResourceType::Mos => Some("mos"),
            ResourceType::Itm => Some("itm"),
            ResourceType::Spl => Some("spl"),
            ResourceType::Bcs => Some("bcs"),
            ResourceType::Ids => Some("ids"),
            ResourceType::Cre => Some("cre"),
            ResourceType::Are => Some("are"),
            ResourceType::Dlg => Some("dlg"),
            ResourceType::Two => Some("two"),
            ResourceType::Gam => Some("gam"),
            ResourceType::Sto => Some("sto"),
            ResourceType::Wmp => Some("wmp"),
            ResourceType::Eff => Some("eff"),
            ResourceType::Bs => Some("bs"),
            ResourceType::Chr => Some("chr"),
            ResourceType::Vvc => Some("vvc"),
            ResourceType::Vef => Some("vef"),
            ResourceType::Pro => Some("pro"),
            ResourceType::Bio => Some("bio"),
            ResourceType::Wbm => Some("wbm"),
            ResourceType::Fnt => Some("fnt"),
            ResourceType::Gui => Some("gui"),
            ResourceType::Sql => Some("sql"),
            ResourceType::Pvrz => Some("pvrz"),
            ResourceType::Glsl => Some("glsl"),
            ResourceType::Tot => Some("tot"),
            ResourceType::Toh => Some("toh"),
            ResourceType::Menu => Some("menu"),
            ResourceType::Lua => Some("lua"),
            ResourceType::Ttf => Some("ttf"),
            ResourceType::Png => Some("png"),
            ResourceType::Bah => Some("bah"),
            ResourceType::Ini => Some("ini"),
            ResourceType::Src => Some("src"),
            ResourceType::Maze => Some("maze"),
            ResourceType::Mus => Some("mus"),
            ResourceType::Acm => Some("acm"),
            ResourceType::Unknown(_) => None,
        }
    }
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

            assert_json_snapshot!(format!("key_file_{i}"), key, {
                ".file" => ""
            });
        }
    }

    #[test]
    fn test_resource_type_roundtrip() {
        for i in 0..16u16.pow(3) {
            assert_eq!(ResourceType::from(i).to_u16(), i);
        }
    }
}
