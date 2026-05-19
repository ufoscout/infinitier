use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::{Wed, WedPolygon};

/// A Wed file exporter.
///
/// Writes a [`Wed`] back to the WED V1.3 binary format used by the Infinity
/// Engine. Section offsets are recomputed from the in-memory model, so the
/// resulting file may not be byte-identical to a file produced by other
/// tools — but its semantic content (and re-importing it) matches.
pub struct WedExporter;

impl WedExporter {
    /// Writes `wed` as WED V1.3 bytes into `writer`.
    pub fn export<W: Write>(&self, wed: &Wed, writer: &mut W) -> io::Result<()> {
        let bytes = build_bytes(wed)?;
        writer.write_all(&bytes)
    }

    /// Writes `wed` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, wed: &Wed, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(wed, &mut writer)?;
        writer.flush()
    }
}

const HEADER_SIZE: u32 = 32;
const OVERLAY_RECORD_SIZE: u32 = 24;
const SECONDARY_HEADER_SIZE: u32 = 20;
const DOOR_RECORD_SIZE: u32 = 26;
const TILEMAP_ENTRY_SIZE: u32 = 10;
const POLYGON_SIZE: u32 = 18;

/// Section order in the emitted file:
/// 1. Primary header
/// 2. Overlay records
/// 3. Secondary header
/// 4. Door records
/// 5. Per-overlay tilemap entries
/// 6. Door tile cells
/// 7. Per-overlay tile index lookup tables
/// 8. Wall groups
/// 9. Wall polygons
/// 10. Per-door open/closed polygons
/// 11. Polytable
/// 12. Vertices
///
/// This mirrors the layout produced by the original Bioware tools (and
/// observed in the BG2 game assets), which keeps round-tripped files
/// recognisable to NearInfinity and similar utilities.
fn build_bytes(wed: &Wed) -> io::Result<Vec<u8>> {
    let n_overlays = wed.overlays.len();
    let n_doors = wed.doors.len();

    let overlays_offset = HEADER_SIZE;
    let secondary_header_offset = overlays_offset + (n_overlays as u32) * OVERLAY_RECORD_SIZE;
    let doors_offset = secondary_header_offset + SECONDARY_HEADER_SIZE;

    let mut cursor = doors_offset + (n_doors as u32) * DOOR_RECORD_SIZE;

    // Tilemap entries, one block per overlay. Empty overlays contribute no
    // bytes; their stored offset matches the cursor at their slot, which
    // happens to equal the next non-empty overlay's offset (or the start of
    // the door tile cells when every later overlay is also empty). This
    // matches the convention observed in vanilla WEDs.
    let mut overlay_tilemap_offsets = Vec::with_capacity(n_overlays);
    for overlay in &wed.overlays {
        overlay_tilemap_offsets.push(cursor);
        cursor += (overlay.tilemap.len() as u32) * TILEMAP_ENTRY_SIZE;
    }

    let door_tiles_offset = cursor;
    cursor += (wed.door_tile_cells.len() as u32) * 2;

    let mut overlay_lookup_offsets = Vec::with_capacity(n_overlays);
    for overlay in &wed.overlays {
        overlay_lookup_offsets.push(cursor);
        cursor += (overlay.tile_index_lookup.len() as u32) * 2;
    }

    let wall_groups_offset = cursor;
    cursor += (wed.wall_groups.len() as u32) * 4;

    let wall_polygons_offset = cursor;
    cursor += (wed.wall_polygons.len() as u32) * POLYGON_SIZE;

    let mut door_polygon_offsets = Vec::with_capacity(n_doors);
    for door in &wed.doors {
        let open_offset = cursor;
        cursor += (door.open_polygons.len() as u32) * POLYGON_SIZE;
        let closed_offset = cursor;
        cursor += (door.closed_polygons.len() as u32) * POLYGON_SIZE;
        door_polygon_offsets.push((open_offset, closed_offset));
    }

    let polytable_offset = cursor;
    cursor += (wed.wall_polygon_indexes.len() as u32) * 2;

    let verticles_offset = cursor;
    cursor += (wed.verticles.len() as u32) * 4;

    let total_size = cursor as usize;
    let mut buf = Vec::with_capacity(total_size);

    // 1. Primary header.
    buf.extend_from_slice(b"WED V1.3");
    buf.extend_from_slice(&(n_overlays as u32).to_le_bytes());
    buf.extend_from_slice(&(n_doors as u32).to_le_bytes());
    buf.extend_from_slice(&overlays_offset.to_le_bytes());
    buf.extend_from_slice(&secondary_header_offset.to_le_bytes());
    buf.extend_from_slice(&doors_offset.to_le_bytes());
    buf.extend_from_slice(&door_tiles_offset.to_le_bytes());

    // 2. Overlay records.
    for (i, overlay) in wed.overlays.iter().enumerate() {
        buf.extend_from_slice(&overlay.width.to_le_bytes());
        buf.extend_from_slice(&overlay.height.to_le_bytes());
        write_fixed_string(&mut buf, &overlay.name.name, 8)?;
        buf.extend_from_slice(&overlay.unique_tiles_count.to_le_bytes());
        buf.extend_from_slice(&overlay.movement_type.to_le_bytes());
        buf.extend_from_slice(&overlay_tilemap_offsets[i].to_le_bytes());
        buf.extend_from_slice(&overlay_lookup_offsets[i].to_le_bytes());
    }

    // 3. Secondary header.
    buf.extend_from_slice(&(wed.wall_polygons.len() as u32).to_le_bytes());
    buf.extend_from_slice(&wall_polygons_offset.to_le_bytes());
    buf.extend_from_slice(&verticles_offset.to_le_bytes());
    buf.extend_from_slice(&wall_groups_offset.to_le_bytes());
    buf.extend_from_slice(&polytable_offset.to_le_bytes());

    // 4. Door records.
    for (i, door) in wed.doors.iter().enumerate() {
        write_fixed_string(&mut buf, &door.name, 8)?;
        buf.extend_from_slice(&door.state.to_u16().to_le_bytes());
        buf.extend_from_slice(&door.door_tile_cell_index.to_le_bytes());
        buf.extend_from_slice(&door.door_tile_cell_count.to_le_bytes());
        buf.extend_from_slice(&(door.open_polygons.len() as u16).to_le_bytes());
        buf.extend_from_slice(&(door.closed_polygons.len() as u16).to_le_bytes());
        buf.extend_from_slice(&door_polygon_offsets[i].0.to_le_bytes());
        buf.extend_from_slice(&door_polygon_offsets[i].1.to_le_bytes());
    }

    // 5. Per-overlay tilemap entries.
    for overlay in &wed.overlays {
        for entry in &overlay.tilemap {
            buf.extend_from_slice(&entry.start_index_in_lookup.to_le_bytes());
            buf.extend_from_slice(&entry.count_in_lookup.to_le_bytes());
            buf.extend_from_slice(&entry.secondary_tile_index.to_le_bytes());
            buf.push(entry.overlay_mask);
            buf.extend_from_slice(&entry.unknown);
        }
    }

    // 6. Door tile cells.
    for cell in &wed.door_tile_cells {
        buf.extend_from_slice(&cell.to_le_bytes());
    }

    // 7. Per-overlay tile index lookup.
    for overlay in &wed.overlays {
        for idx in &overlay.tile_index_lookup {
            buf.extend_from_slice(&idx.to_le_bytes());
        }
    }

    // 8. Wall groups.
    for wg in &wed.wall_groups {
        buf.extend_from_slice(&wg.polygon_index.to_le_bytes());
        buf.extend_from_slice(&wg.polygon_count.to_le_bytes());
    }

    // 9. Wall polygons.
    for poly in &wed.wall_polygons {
        write_polygon(&mut buf, poly);
    }

    // 10. Per-door open/closed polygons.
    for door in &wed.doors {
        for poly in &door.open_polygons {
            write_polygon(&mut buf, poly);
        }
        for poly in &door.closed_polygons {
            write_polygon(&mut buf, poly);
        }
    }

    // 11. Polytable.
    for idx in &wed.wall_polygon_indexes {
        buf.extend_from_slice(&idx.to_le_bytes());
    }

    // 12. Vertices.
    for v in &wed.verticles {
        buf.extend_from_slice(&v.x.to_le_bytes());
        buf.extend_from_slice(&v.y.to_le_bytes());
    }

    debug_assert_eq!(buf.len(), total_size);
    Ok(buf)
}

fn write_fixed_string(buf: &mut Vec<u8>, s: &str, size: usize) -> io::Result<()> {
    let bytes = s.as_bytes();
    if bytes.len() > size {
        return Err(io::Error::other(format!(
            "name {s:?} too long ({} bytes) for {}-byte field",
            bytes.len(),
            size
        )));
    }
    buf.extend_from_slice(bytes);
    buf.resize(buf.len() + (size - bytes.len()), 0);
    Ok(())
}

fn write_polygon(buf: &mut Vec<u8>, poly: &WedPolygon) {
    buf.extend_from_slice(&poly.vertex_index.to_le_bytes());
    buf.extend_from_slice(&poly.vertex_count.to_le_bytes());
    buf.push(poly.flags.bits());
    buf.extend_from_slice(&poly.height.to_le_bytes());
    buf.extend_from_slice(&poly.min_x.to_le_bytes());
    buf.extend_from_slice(&poly.max_x.to_le_bytes());
    buf.extend_from_slice(&poly.min_y.to_le_bytes());
    buf.extend_from_slice(&poly.max_y.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WedImporter;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    #[test]
    fn test_export_roundtrip() {
        let path = get_assets_path()
            .join("KEY/bg2")
            .join("override")
            .join("ar0072.WED");
        let original = WedImporter { name: "wed_test" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();

        let mut buf: Vec<u8> = Vec::new();
        WedExporter.export(&original, &mut buf).unwrap();

        let re_imported = WedImporter { name: "wed_test_rt" }
            .import(&DataSource::new(buf))
            .unwrap();

        assert_eq!(original, re_imported);
    }

    #[test]
    fn test_export_byte_identical_to_source() {
        // Bonus check: for files produced by the original Bioware tools, our
        // layout matches theirs exactly, so the exported bytes equal the
        // source file's bytes.
        let path = get_assets_path()
            .join("KEY/bg2")
            .join("override")
            .join("ar0072.WED");
        let original_bytes = std::fs::read(&path).unwrap();
        let wed = WedImporter { name: "wed_test" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();

        let mut buf: Vec<u8> = Vec::new();
        WedExporter.export(&wed, &mut buf).unwrap();

        assert_eq!(buf.len(), original_bytes.len());
        assert_eq!(buf, original_bytes);
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let path = get_assets_path()
            .join("KEY/bg2")
            .join("override")
            .join("ar0072.WED");
        let original = WedImporter { name: "wed_test" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        WedExporter.export_to_file(&original, tmp.path()).unwrap();

        let re_imported = WedImporter { name: "wed_test_rt" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();

        assert_eq!(original, re_imported);
    }
}
