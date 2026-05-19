#![doc = include_str!("../readme.md")]

use infinitier_common::ResourceType;
use serde::{Deserialize, Serialize};

mod exporter;
mod importer;

pub use exporter::WedExporter;
pub use importer::WedImporter;

/// Represents a Wed file.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wed {
    pub overlays: Vec<WedOverlay>,
    pub doors: Vec<WedDoor>,
    pub wall_polygons: Vec<WedPolygon>,
    pub wall_groups: Vec<WedWallGroup>,
    pub wall_polygon_indexes: Vec<u16>,
    pub verticles: Vec<WedVertex>,
    pub door_tile_cells: Vec<u16>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceReference {
    pub name: String,
    pub r#type: ResourceType,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedOverlay {
    pub width: u16,
    pub height: u16,
    pub name: ResourceReference,
    // Only used in Enhanced Editions
    pub unique_tiles_count: u16,
    // Only used in Enhanced Editions
    // Values: ["Default", "Disable rendering", "Alternate rendering"]
    pub movement_type: u16,
    /// One entry per overlay cell: `width * height` entries when the overlay
    /// is used, empty for the unused secondary overlay slots that fill out
    /// the 5-slot fixed table.
    pub tilemap: Vec<WedTilemapEntry>,
    /// Flat table of TIS tile indices referenced by [`WedTilemapEntry`]s of
    /// this overlay through `start_index_in_lookup` / `count_in_lookup`.
    pub tile_index_lookup: Vec<u16>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedTilemapEntry {
    /// Start index into the owning overlay's `tile_index_lookup`.
    pub start_index_in_lookup: u16,
    /// Number of consecutive lookup entries that belong to this cell
    /// (animation frames cycle through them).
    pub count_in_lookup: u16,
    /// TIS tile index used as the alternate state for this cell
    /// (e.g. lit/night), or `-1` when there is none.
    pub secondary_tile_index: i16,
    /// Bitmask of overlays drawn on top of this cell.
    pub overlay_mask: u8,
    /// Three trailing bytes the engine treats as unknown/padding. Preserved
    /// verbatim so round-tripping is byte-exact.
    pub unknown: [u8; 3],
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedDoor {
    pub name: String,
    pub state: WedDoorState,
    pub door_tile_cell_index: u16,
    pub door_tile_cell_count: u16,
    pub open_polygons: Vec<WedPolygon>,
    pub closed_polygons: Vec<WedPolygon>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WedDoorState {
    Open,
    Closed,
}

impl WedDoorState {
    pub fn from_u16(state: u16) -> std::io::Result<WedDoorState> {
        match state {
            0 => Ok(WedDoorState::Open),
            1 => Ok(WedDoorState::Closed),
            val => Err(std::io::Error::other(format!("Invalid door state: {val}"))),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            WedDoorState::Open => 0,
            WedDoorState::Closed => 1,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedPolygon {
    pub vertex_index: u32,
    pub vertex_count: u32,
    pub flags: WedPolygonFlag,
    pub height: i8,
    pub min_x: i16,
    pub max_x: i16,
    pub min_y: i16,
    pub max_y: i16,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct WedPolygonFlag: u8 {
        const ShadeWall =       1 << 0;
        const SemiTransparent = 1 << 1;
        const HoveringWall = 1 << 2;
        const CoverAnimations = 1 << 3;
        const Null = 1 << 4 | 1 << 5 | 1 << 6;
        const Door = 1 << 7;
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedWallGroup {
    pub polygon_index: u16,
    pub polygon_count: u16,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedVertex {
    pub x: i16,
    pub y: i16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wed_poligon_flag() {
        assert_eq!(
            WedPolygonFlag::empty(),
            WedPolygonFlag::from_bits(0).unwrap()
        );
        assert_eq!(
            WedPolygonFlag::empty(),
            WedPolygonFlag::from_bits_truncate(0)
        );
        assert_eq!(
            WedPolygonFlag::ShadeWall,
            WedPolygonFlag::from_bits(1).unwrap()
        );
        assert_eq!(
            WedPolygonFlag::ShadeWall,
            WedPolygonFlag::from_bits_truncate(1)
        );
        assert_eq!(
            WedPolygonFlag::CoverAnimations.union(WedPolygonFlag::ShadeWall),
            WedPolygonFlag::from_bits(9).unwrap()
        );
        assert_eq!(
            WedPolygonFlag::CoverAnimations.union(WedPolygonFlag::ShadeWall),
            WedPolygonFlag::from_bits_truncate(9)
        );
    }
}
