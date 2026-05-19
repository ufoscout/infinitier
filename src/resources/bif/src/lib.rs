#![doc = include_str!("../readme.md")]

use infinitier_common::ResourceType;
use infinitier_datasource::DataSource;

mod exporter;
mod importer;

pub use exporter::BifExporter;
pub use importer::{BifImporter, detect_biff_type};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    Biff, // BIFF V1
    Bif,  // BIF V1.0   (single zlib stream)
    Bifc, // BIFC V1.0  (chunked zlib)
}

pub const BIFFV1_SIGNATURE: &str = "BIFFV1  ";
pub const BIF_V1_0_SIGNATURE: &str = "BIF V1.0";
pub const BIFCV1_0_SIGNATURE: &str = "BIFCV1.0";

impl Type {
    pub fn signature(&self) -> &'static str {
        match self {
            Type::Biff => BIFFV1_SIGNATURE,
            Type::Bif => BIF_V1_0_SIGNATURE,
            Type::Bifc => BIFCV1_0_SIGNATURE,
        }
    }
}

#[derive(Debug)]
pub struct Bif {
    pub name: String,
    pub r#type: Type,
    pub resources: Vec<BifEmbeddedResource>,
    /// DataSource for reading embedded resource data.
    /// For uncompressed BIFF V1 this points to the original file;
    /// for compressed formats it points to the in-memory decompressed bytes.
    pub datasource: DataSource,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BifEmbeddedResource {
    File {
        locator: u32,
        size: u32,
        offset: u64,
        r#type: ResourceType,
    },
    Tileset {
        locator: u32,
        size: u32,
        count: u32,
        offset: u64,
        r#type: ResourceType,
    },
}
