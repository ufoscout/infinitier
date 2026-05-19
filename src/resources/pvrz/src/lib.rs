#![doc = include_str!("../readme.md")]

// To decode PVR texture files check: https://crates.io/crates/texture2ddecoder
// To encode them back: https://crates.io/crates/texpresso

use image::{ImageBuffer, Rgba};
use log::error;

mod exporter;
mod importer;

pub use exporter::PvrzExporter;
pub use importer::PvrzImporter;

/// A fully-decoded PVRZ texture: the parsed header plus the RGBA8 image
/// produced by decompressing its DXT1/DXT5 payload.
#[derive(Debug)]
pub struct Pvrz {
    pub header: PvrzHeader,
    pub image: ImageBuffer<Rgba<u8>, Vec<u8>>,
}

/// A PVR header
#[derive(Debug, PartialEq, Eq)]
pub struct PvrzHeader {
    pub version: u32,
    pub flags: u32,
    pub pixel_format: PvrDataCompression,
    pub color_space: u32,
    pub channel_type: u32,
    pub height: u32,
    pub width: u32,
    pub depth: u32,
    pub surfaces_number: u32,
    pub faces_number: u32,
    pub mip_map_count: u32,
    pub metadata_size: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PvrDataCompression {
    /// DXT1 aka BC1 compressed texture
    DXT1,
    /// DXT5 aka BC3 compressed texture
    DXT5,
}

impl PvrDataCompression {
    /// Converts a u64 value to a `PvrDataCompression` enum variant.
    pub fn from_u64(value: u64) -> std::io::Result<PvrDataCompression> {
        match value {
            7 => Ok(PvrDataCompression::DXT1),
            11 => Ok(PvrDataCompression::DXT5),
            _ => {
                error!("Unexpected PVRZ pixel_format: {}", value);
                Err(std::io::Error::other(format!(
                    "Unexpected pixel_format: {}",
                    value
                )))
            }
        }
    }

    /// Converts a `PvrDataCompression` enum variant to a u32 value
    pub fn to_u64(&self) -> u64 {
        match self {
            PvrDataCompression::DXT1 => 7,
            PvrDataCompression::DXT5 => 11,
        }
    }
}
