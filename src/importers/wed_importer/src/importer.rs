use std::io::{Seek, SeekFrom};

use infinitier_common::ResourceType;
use infinitier_datasource::{DataSource, Importer, ReadExt, SeekExt};
use log::{debug, error};

use crate::{
    ResourceReference, Wed, WedDoor, WedDoorState, WedOverlay, WedPolygon, WedPolygonFlag,
    WedTilemapEntry, WedVertex, WedWallGroup,
};

/// A Wed file importer
pub struct WedImporter<'a> {
    pub name: &'a str,
}

impl Importer for WedImporter<'_> {
    type T = Wed;

    fn import(&self, source: &DataSource) -> std::io::Result<Wed> {
        let mut reader = source.reader()?;

        let signature = reader.read_string(8)?;

        if signature != "WED V1.3" {
            error!(
                "Not a WED V1.3 file ({}): signature={:?}",
                self.name, signature
            );
            return Err(std::io::Error::other("Wrong file type"));
        }

        let overlays_size = reader.read_u32()? as usize;
        let doors_size = reader.read_u32()? as usize;
        let overlays_offset = reader.read_u32()? as u64;
        let secondary_header_offset = reader.read_u32()? as u64;
        let doors_offset = reader.read_u32()? as u64;
        let door_tiles_offset = reader.read_u32()? as u64;

        // Read overlay records. Tilemap + lookup payload is read in a
        // second pass once we know all section boundaries.
        struct OverlayRecord {
            width: u16,
            height: u16,
            name: ResourceReference,
            unique_tiles_count: u16,
            movement_type: u16,
            tilemap_offset: u64,
            tile_index_lookup_offset: u64,
        }
        let mut overlay_records = Vec::with_capacity(overlays_size);
        reader.set_position(overlays_offset)?;
        for _ in 0..overlays_size {
            overlay_records.push(OverlayRecord {
                width: reader.read_u16()?,
                height: reader.read_u16()?,
                name: ResourceReference {
                    name: reader.read_string(8)?,
                    r#type: ResourceType::Tis,
                },
                unique_tiles_count: reader.read_u16()?,
                movement_type: reader.read_u16()?,
                tilemap_offset: reader.read_u32()? as u64,
                tile_index_lookup_offset: reader.read_u32()? as u64,
            });
        }

        // Secondary header.
        reader.set_position(secondary_header_offset)?;
        let wall_polygons_count = reader.read_u32()? as usize;
        let polygons_offset = reader.read_u32()? as u64;
        let verticles_offset = reader.read_u32()? as u64;
        let wall_groups_offset = reader.read_u32()? as u64;
        let polytable_offset = reader.read_u32()? as u64;

        // Door records. Same deal as overlays: store the offsets, read the
        // open/closed polygon arrays in a later pass.
        struct DoorRecord {
            name: String,
            state: WedDoorState,
            door_tile_cell_index: u16,
            door_tile_cell_count: u16,
            open_count: u16,
            closed_count: u16,
            open_offset: u64,
            closed_offset: u64,
        }
        let mut door_records = Vec::with_capacity(doors_size);
        let mut door_tile_cells_count = 0usize;
        reader.set_position(doors_offset)?;
        for _ in 0..doors_size {
            let door = DoorRecord {
                name: reader.read_string(8)?,
                state: WedDoorState::from_u16(reader.read_u16()?)?,
                door_tile_cell_index: reader.read_u16()?,
                door_tile_cell_count: reader.read_u16()?,
                open_count: reader.read_u16()?,
                closed_count: reader.read_u16()?,
                open_offset: reader.read_u32()? as u64,
                closed_offset: reader.read_u32()? as u64,
            };
            door_tile_cells_count += door.door_tile_cell_count as usize;
            door_records.push(door);
        }

        // Wall polygons.
        let mut wall_polygons = Vec::with_capacity(wall_polygons_count);
        reader.set_position(polygons_offset)?;
        for _ in 0..wall_polygons_count {
            wall_polygons.push(read_polygon(&mut reader)?);
        }

        // Section-end derivation (matches NearInfinity's "next offset"
        // approach). Used for wall groups and vertices, neither of which
        // store an explicit count anywhere.
        let file_size = reader.seek(SeekFrom::End(0))?;
        let mut door_offsets_with_data: Vec<u64> = door_records
            .iter()
            .flat_map(|d| {
                let mut v = Vec::with_capacity(2);
                if d.open_count > 0 {
                    v.push(d.open_offset);
                }
                if d.closed_count > 0 {
                    v.push(d.closed_offset);
                }
                v
            })
            .collect();
        let mut section_starts: Vec<u64> = vec![
            overlays_offset,
            secondary_header_offset,
            doors_offset,
            door_tiles_offset,
            polygons_offset,
            verticles_offset,
            wall_groups_offset,
            polytable_offset,
            file_size,
        ];
        section_starts.append(&mut door_offsets_with_data);
        section_starts.sort();
        section_starts.dedup();
        let next_after = |start: u64| -> u64 {
            section_starts
                .iter()
                .find(|&&o| o > start)
                .copied()
                .unwrap_or(file_size)
        };

        // Wall groups: count derived from byte range.
        let wall_group_count = ((next_after(wall_groups_offset) - wall_groups_offset) / 4) as usize;
        let mut wall_groups = Vec::with_capacity(wall_group_count);
        let mut polytable_count = 0usize;
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

        // Polytable.
        let mut wall_polygon_indexes = Vec::with_capacity(polytable_count);
        reader.set_position(polytable_offset)?;
        for _ in 0..polytable_count {
            wall_polygon_indexes.push(reader.read_u16()?);
        }

        // Vertices: count derived from byte range. The previous importer
        // summed wall-polygon vertex counts, which undercounts because
        // door polygons share the same vertex pool.
        let verticles_count = ((next_after(verticles_offset) - verticles_offset) / 4) as usize;
        let mut verticles = Vec::with_capacity(verticles_count);
        reader.set_position(verticles_offset)?;
        for _ in 0..verticles_count {
            verticles.push(WedVertex {
                x: reader.read_i16()?,
                y: reader.read_i16()?,
            });
        }

        // Door tile cells.
        let mut door_tile_cells = Vec::with_capacity(door_tile_cells_count);
        reader.set_position(door_tiles_offset)?;
        for _ in 0..door_tile_cells_count {
            door_tile_cells.push(reader.read_u16()?);
        }

        // Per-overlay tilemap entries and tile index lookup tables.
        let mut overlays = Vec::with_capacity(overlay_records.len());
        for rec in overlay_records {
            let cells = rec.width as usize * rec.height as usize;
            let mut tilemap = Vec::with_capacity(cells);
            if cells > 0 {
                reader.set_position(rec.tilemap_offset)?;
                for _ in 0..cells {
                    tilemap.push(WedTilemapEntry {
                        start_index_in_lookup: reader.read_u16()?,
                        count_in_lookup: reader.read_u16()?,
                        secondary_tile_index: reader.read_i16()?,
                        overlay_mask: reader.read_u8()?,
                        unknown: [reader.read_u8()?, reader.read_u8()?, reader.read_u8()?],
                    });
                }
            }
            let lookup_count = tilemap.iter().map(|e| e.count_in_lookup as usize).sum();
            let mut tile_index_lookup = Vec::with_capacity(lookup_count);
            if lookup_count > 0 {
                reader.set_position(rec.tile_index_lookup_offset)?;
                for _ in 0..lookup_count {
                    tile_index_lookup.push(reader.read_u16()?);
                }
            }
            overlays.push(WedOverlay {
                width: rec.width,
                height: rec.height,
                name: rec.name,
                unique_tiles_count: rec.unique_tiles_count,
                movement_type: rec.movement_type,
                tilemap,
                tile_index_lookup,
            });
        }

        // Per-door open/closed polygons.
        let mut doors = Vec::with_capacity(door_records.len());
        for rec in door_records {
            let mut open_polygons = Vec::with_capacity(rec.open_count as usize);
            if rec.open_count > 0 {
                reader.set_position(rec.open_offset)?;
                for _ in 0..rec.open_count {
                    open_polygons.push(read_polygon(&mut reader)?);
                }
            }
            let mut closed_polygons = Vec::with_capacity(rec.closed_count as usize);
            if rec.closed_count > 0 {
                reader.set_position(rec.closed_offset)?;
                for _ in 0..rec.closed_count {
                    closed_polygons.push(read_polygon(&mut reader)?);
                }
            }
            doors.push(WedDoor {
                name: rec.name,
                state: rec.state,
                door_tile_cell_index: rec.door_tile_cell_index,
                door_tile_cell_count: rec.door_tile_cell_count,
                open_polygons,
                closed_polygons,
            });
        }

        debug!(
            "Loaded {} [WED]: {} overlays, {} doors, {} wall polygons",
            self.name,
            overlays.len(),
            doors.len(),
            wall_polygons.len()
        );
        Ok(Wed {
            overlays,
            doors,
            wall_polygons,
            wall_groups,
            wall_polygon_indexes,
            verticles,
            door_tile_cells,
        })
    }
}

fn read_polygon<R: ReadExt>(reader: &mut R) -> std::io::Result<WedPolygon> {
    Ok(WedPolygon {
        vertex_index: reader.read_u32()?,
        vertex_count: reader.read_u32()?,
        flags: WedPolygonFlag::from_bits_truncate(reader.read_u8()?),
        height: reader.read_i8()?,
        min_x: reader.read_i16()?,
        max_x: reader.read_i16()?,
        min_y: reader.read_i16()?,
        max_y: reader.read_i16()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::{get_assets_path, parse_json_file};

    #[test]
    #[ignore = "fixture regeneration helper; run manually after schema changes"]
    fn regenerate_ar0072_json_fixture() {
        let path = get_assets_path().join("KEY/bg2").join("override");
        let wed_path = path.join("ar0072.WED");
        let json_path = path.join("ar0072.json");
        let wed = WedImporter { name: "fixture_regen" }
            .import(&DataSource::new(wed_path.as_path()))
            .unwrap();
        let json = serde_json::to_string_pretty(&wed).unwrap();
        std::fs::write(&json_path, json).unwrap();
    }

    #[test]
    fn test_parse_wed_file() {
        let path = get_assets_path().join("KEY/bg2").join("override");
        let wed_path = path.join("ar0072.WED");
        let json_path = path.join("ar0072.json");

        let expected: Wed = parse_json_file(&json_path);

        let actual = WedImporter { name: "wed_test" }
            .import(&DataSource::new(wed_path.as_path()))
            .unwrap();

        assert_eq!(actual.overlays.len(), 5);
        assert_eq!(actual.doors.len(), 2);
        assert_eq!(actual.wall_polygons.len(), 94);
        assert_eq!(actual.wall_groups.len(), 16);
        assert_eq!(actual.wall_polygon_indexes.len(), 125);
        // Door polygons share the vertex pool with wall polygons. The
        // previous importer summed only wall vertex counts (2191); the
        // section actually contains 2211 vertices.
        assert_eq!(actual.verticles.len(), 2211);
        assert_eq!(actual.door_tile_cells.len(), 11);
        // First overlay is the only populated one (40x30 = 1200 cells).
        assert_eq!(actual.overlays[0].tilemap.len(), 1200);
        assert_eq!(actual.overlays[0].tile_index_lookup.len(), 1200);
        for overlay in actual.overlays.iter().skip(1) {
            assert!(overlay.tilemap.is_empty());
            assert!(overlay.tile_index_lookup.is_empty());
        }
        // Door 1: 1 open + 1 closed; Door 2: 2 open + 1 closed.
        assert_eq!(actual.doors[0].open_polygons.len(), 1);
        assert_eq!(actual.doors[0].closed_polygons.len(), 1);
        assert_eq!(actual.doors[1].open_polygons.len(), 2);
        assert_eq!(actual.doors[1].closed_polygons.len(), 1);

        assert_eq!(actual, expected, "wed mismatch for ar0072.WED");
    }
}
