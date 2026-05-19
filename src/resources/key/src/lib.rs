#![doc = include_str!("../readme.md")]

use serde::{Deserialize, Serialize};

mod exporter;
mod importer;

pub use exporter::KeyExporter;
pub use importer::KeyImporter;

pub use infinitier_common::ResourceType;

/// A KEY file (CHITIN.KEY)
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub signature: String,
    pub version: String,
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
    pub fn from(bit: u16) -> Self {
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

#[cfg(test)]
mod tests {
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
}
