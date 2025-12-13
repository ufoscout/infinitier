use serde::{Deserialize, Serialize};

use crate::{
    datasource::{DataSource, Importer},
    resource::key::ResourceType,
};

/// A Wed file importer
pub struct WedImporter;

impl Importer for WedImporter {
    type T = Wed;

    fn import(source: &DataSource) -> std::io::Result<Wed> {
        let mut reader = source.reader()?;

        let signature = reader.read_string(8)?;

        if signature != "WED V1.3" {
            return Err(std::io::Error::other("Wrong file type"));
        }

        let overlays_size = reader.read_u32()? as usize;
        let doors_size = reader.read_u32()? as usize;
        let overlays_offset = reader.read_u32()? as u64;
        let secondary_header_offset = reader.read_u32()? as u64;
        let doors_offset = reader.read_u32()? as u64;
        let door_tiles_offset = reader.read_u32()? as u64;

        // Read overlays
        let mut overlays = Vec::with_capacity(overlays_size);
        {
            reader.set_position(overlays_offset)?;
            for _ in 0..overlays_size {
                overlays.push(WedOverlay {
                    width: reader.read_u16()?,
                    height: reader.read_u16()?,
                    name: ResourceReference {
                        name: reader.read_string(8)?,
                        r#type: ResourceType::Tis,
                    },
                    unique_tiles_count: reader.read_u16()?,
                    movement_type: reader.read_u16()?,
                    tile_index_lookup_offset: reader.read_u32()? as u64,
                    tilemap_offset: reader.read_u32()? as u64,
                });
            }
        }

        // Read secondary Header

        reader.set_position(secondary_header_offset)?;
        let wall_polygons_count = reader.read_u32()? as usize;
        let polygons_offset = reader.read_u32()? as u64;
        let verticles_offset = reader.read_u32()? as u64;
        let wall_groups_offset = reader.read_u32()? as u64;
        let polytable_offset = reader.read_u32()? as u64;

        // Read Doors
        let mut doors = Vec::with_capacity(doors_size);
        let mut door_tile_cells_count = 0;
        {
            reader.set_position(doors_offset)?;
            for _ in 0..doors_size {
                let door = WedDoor {
                    name: reader.read_string(8)?,
                    state: WedDoorState::from_u16(reader.read_u16()?)?,
                    door_tile_cell_index: reader.read_u16()?,
                    door_tile_cell_count: reader.read_u16()?,
                    polygon_open_state_count: reader.read_u16()?,
                    polygon_closed_state_count: reader.read_u16()?,
                    polygon_open_state_offset: reader.read_u32()? as u64,
                    polygon_closed_state_offset: reader.read_u32()? as u64,
                };
                door_tile_cells_count += door.door_tile_cell_count as usize;
                doors.push(door);
            }
        }

        // Read Polygons
        let mut polygons = Vec::with_capacity(wall_polygons_count);
        let mut verticles_count = 0;
        {
            reader.set_position(polygons_offset)?;
            for _ in 0..wall_polygons_count {
                let polygon = WedPolygon {
                    vertex_index: reader.read_u32()?,
                    vertex_count: reader.read_u32()?,
                    flags: WedPolygonFlag::from_bits_truncate(reader.read_u8()?),
                    height: reader.read_i8()?,
                    min_x: reader.read_i16()?,
                    max_x: reader.read_i16()?,
                    min_y: reader.read_i16()?,
                    max_y: reader.read_i16()?,
                };
                verticles_count += polygon.vertex_count as usize;
                polygons.push(polygon);
            }
        }

        // Read Wall Groups
        let wall_group_count = overlays[0].width as usize * overlays[0].height as usize / 75;
        let mut wall_groups = Vec::with_capacity(wall_group_count);
        let mut polytable_count = 0;
        {
            reader.set_position(wall_groups_offset)?;
            for _ in 0..wall_group_count {
                let wall = WedWallGroup {
                    polygon_index: reader.read_u16()?,
                    polygon_count: reader.read_u16()?,
                };
                polytable_count =
                    polytable_count.max(wall.polygon_count as usize + wall.polygon_index as usize);
                wall_groups.push(wall);
            }
        }

        // Read Polytable
        let mut wall_polygon_indexes = Vec::with_capacity(polytable_count);
        {
            reader.set_position(polytable_offset)?;
            for _ in 0..polytable_count {
                wall_polygon_indexes.push(reader.read_u16()?);
            }
        }

        // Read Verticles
        let mut verticles = Vec::with_capacity(verticles_count);
        {
            reader.set_position(verticles_offset)?;
            for _ in 0..verticles_count {
                verticles.push(WedVertex {
                    x: reader.read_i16()?,
                    y: reader.read_i16()?,
                });
            }
        }

        // Read Door Tile Cells
        let mut door_tile_cells = Vec::with_capacity(door_tile_cells_count);
        {
            reader.set_position(door_tiles_offset)?;
            for _ in 0..door_tile_cells_count {
                door_tile_cells.push(reader.read_u16()?);
            }
        }

        Ok(Wed {
            overlays,
            doors,
            polygons,
            wall_groups,
            wall_polygon_indexes,
            verticles,
            door_tile_cells,
        })
    }
}

/// Represents a Wed file.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wed {
    pub overlays: Vec<WedOverlay>,
    pub doors: Vec<WedDoor>,
    pub polygons: Vec<WedPolygon>,
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
    pub tilemap_offset: u64,
    pub tile_index_lookup_offset: u64,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WedDoor {
    pub name: String,
    pub state: WedDoorState,
    pub door_tile_cell_index: u16,
    pub door_tile_cell_count: u16,
    pub polygon_open_state_count: u16,
    pub polygon_closed_state_count: u16,
    pub polygon_open_state_offset: u64,
    pub polygon_closed_state_offset: u64,
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
    use insta::assert_json_snapshot;

    use super::*;
    use crate::{
        fs::{CaseInsensitiveFS, CaseInsensitivePath},
        test_utils::BG2_RESOURCES_DIR,
    };

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

    #[test]
    fn test_parse_wed_file() {
        let path = CaseInsensitiveFS::new(BG2_RESOURCES_DIR)
            .unwrap()
            .get_path(&CaseInsensitivePath::new("override/ar0072.WED"))
            .unwrap();
        let wed = WedImporter::import(&DataSource::new(path)).unwrap();

        assert_eq!(wed.overlays.len(), 5);
        assert_eq!(wed.doors.len(), 2);
        assert_eq!(wed.polygons.len(), 94);
        assert_eq!(wed.wall_groups.len(), 16);
        assert_eq!(wed.wall_polygon_indexes.len(), 125);
        assert_eq!(wed.verticles.len(), 2191);
        assert_eq!(wed.door_tile_cells.len(), 11);

        assert_json_snapshot!(wed);
    }
}
