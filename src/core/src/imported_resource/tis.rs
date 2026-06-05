//! Eagerly-decoded TIS tileset. Tiles are 64×64 RGBA buffers; the viewer
//! composites them at any user-chosen column count, so the only thing
//! kept around per tile is the pixel buffer itself.

use std::collections::HashMap;
use std::io;

use image::{ImageBuffer, Rgba};
use infinitier_common::ResourceType;
use infinitier_datasource::Importer;
use infinitier_pvrz_resource::{Pvrz, PvrzImporter};
use infinitier_tis_resource::{
    TILE_DIMENSION, Tis, TisPalette, TisPvrz, TisPvrzTile, Type as TisType,
};
use infinitier_wed_resource::WedImporter;

use crate::game::GameData;

/// Default columns count when no WED is registered for the TIS — matches
/// NearInfinity's `DEFAULT_COLUMNS` fallback.
pub const DEFAULT_TILES_PER_ROW: u32 = 5;

/// Bytes per decoded tile (`64 * 64 * 4` RGBA).
const TILE_PIXEL_BYTES: usize = (TILE_DIMENSION as usize) * (TILE_DIMENSION as usize) * 4;

/// A preloaded TIS file: every tile's pixels are decoded ahead of time
/// into a single contiguous buffer, plus the "expected" tiles-per-row
/// value derived from the area's WED (if any).
#[derive(Debug, Clone)]
pub struct ImportedTis {
    /// Tile side length in pixels (always 64).
    pub tile_dimension: u32,
    /// Number of tiles in the tileset.
    pub tile_count: usize,
    /// Decoded RGBA pixels for every tile, packed back-to-back.
    /// Tile `i` occupies `i * 64 * 64 * 4 .. (i + 1) * 64 * 64 * 4`.
    pub tile_pixels: Vec<u8>,
    /// Tiles-per-row to use as the slider's initial value. Sourced from
    /// the primary overlay of the area's WED when present, otherwise
    /// `min(tile_count, 5)` — same fallback as NearInfinity.
    pub expected_columns: u32,
    /// Original on-disk variant — surfaced in the viewer's info bar.
    pub variant: TisType,
}

impl ImportedTis {
    /// Decode every tile up front and look up the area's WED to pick a
    /// default columns count.
    ///
    /// `name` is the TIS resource name (lowercased, no extension —
    /// matches the [`GameData`] indexing convention) and is used to
    /// derive both the WED name (`<name>.wed`) and the PVRZ page names
    /// for the PVRZ-variant tiles (`<name[0]><name[2..]><NN>.pvrz`,
    /// per NearInfinity's `TisV2Decoder.pvrzNameBase`).
    pub fn load(tis: Tis, game_data: &GameData, name: &str) -> io::Result<Self> {
        let (tile_count, tile_pixels, variant) = match &tis {
            Tis::Palette(p) => (p.tiles.len(), decode_palette(p), TisType::Palette),
            Tis::Pvrz(p) => (
                p.tiles.len(),
                decode_pvrz(p, game_data, name)?,
                TisType::Pvrz,
            ),
        };
        let expected_columns = expected_columns_from_wed(name, game_data)
            .unwrap_or_else(|| (tile_count as u32).clamp(1, DEFAULT_TILES_PER_ROW));

        Ok(ImportedTis {
            tile_dimension: TILE_DIMENSION,
            tile_count,
            tile_pixels,
            expected_columns,
            variant,
        })
    }

    /// Composites every tile into a single RGBA image laid out
    /// row-major in a `columns × ceil(tile_count / columns)` grid.
    ///
    /// The last row is padded with fully-transparent pixels when
    /// `tile_count` is not a multiple of `columns`. Panics if
    /// `columns == 0`.
    pub fn compose(&self, columns: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        assert!(columns > 0, "columns must be > 0");
        let rows = (self.tile_count as u32).div_ceil(columns);
        let canvas_w = columns * self.tile_dimension;
        let canvas_h = rows * self.tile_dimension;
        let mut canvas = vec![0u8; (canvas_w * canvas_h * 4) as usize];

        for idx in 0..self.tile_count {
            let col = (idx as u32) % columns;
            let row = (idx as u32) / columns;
            let x0 = col * self.tile_dimension;
            let y0 = row * self.tile_dimension;
            let src = &self.tile_pixels[idx * TILE_PIXEL_BYTES..(idx + 1) * TILE_PIXEL_BYTES];
            for ty in 0..self.tile_dimension {
                let dy = y0 + ty;
                let src_start = (ty * self.tile_dimension * 4) as usize;
                let src_end = src_start + (self.tile_dimension * 4) as usize;
                let dst_start = ((dy * canvas_w + x0) * 4) as usize;
                let dst_end = dst_start + (self.tile_dimension * 4) as usize;
                canvas[dst_start..dst_end].copy_from_slice(&src[src_start..src_end]);
            }
        }

        ImageBuffer::from_vec(canvas_w, canvas_h, canvas)
            .expect("canvas vec sized exactly for canvas_w × canvas_h RGBA")
    }
}

/// Walk the palette tiles and produce a single contiguous RGBA buffer.
fn decode_palette(p: &TisPalette) -> Vec<u8> {
    let mut out = Vec::with_capacity(p.tiles.len() * TILE_PIXEL_BYTES);
    for idx in 0..p.tiles.len() {
        // `tile_image` mirrors NearInfinity's transparency rule
        // (palette index 0 + RGB(0,255,0) ⇒ transparent), so we don't
        // need to reimplement it here.
        let img = p.tile_image(idx);
        out.extend_from_slice(img.as_raw());
    }
    out
}

/// Resolve every PVRZ tile's source rectangle and copy a 64×64 region
/// into the output buffer. PVRZ pages are cached so each is decoded at
/// most once even when many tiles share an atlas.
///
/// "No source" tiles (`pvrz_page == -1`) are painted as **opaque
/// black** — matching NearInfinity's `TisV2Decoder` and the
/// vanilla-engine output. (The reference PNGs in `extracted_resources`
/// were verified empirically: every `-1` tile renders as RGBA
/// `(0, 0, 0, 255)`, not as transparent.)
fn decode_pvrz(p: &TisPvrz, game_data: &GameData, name: &str) -> io::Result<Vec<u8>> {
    let mut pvrz_cache: HashMap<i32, Pvrz> = HashMap::new();
    // Start every byte at 0, then bump alpha to 0xFF in stride. This is
    // the opaque-black state for any tile that doesn't get overwritten
    // by a `copy_pvrz_tile` call below (i.e. the `-1` blanks). For
    // tiles that *do* get overwritten, the PVRZ payload supplies its
    // own alpha and the pre-filled value is discarded — harmless.
    let mut out = vec![0u8; p.tiles.len() * TILE_PIXEL_BYTES];
    for px in out.chunks_exact_mut(4) {
        px[3] = 0xFF;
    }

    for (idx, tile) in p.tiles.iter().enumerate() {
        if tile.is_blank() {
            continue;
        }
        let pvrz = get_or_load_pvrz(&mut pvrz_cache, name, tile, game_data)?;
        let tile_slice = &mut out[idx * TILE_PIXEL_BYTES..(idx + 1) * TILE_PIXEL_BYTES];
        copy_pvrz_tile(tile, pvrz, tile_slice);
    }

    Ok(out)
}

fn get_or_load_pvrz<'a>(
    cache: &'a mut HashMap<i32, Pvrz>,
    tis_name: &str,
    tile: &TisPvrzTile,
    game_data: &GameData,
) -> io::Result<&'a Pvrz> {
    if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(tile.pvrz_page) {
        let pvrz_filename = TisPvrz::pvrz_name_for(tis_name, tile.pvrz_page).ok_or_else(|| {
            io::Error::other(format!(
                "Cannot derive PVRZ name for TIS '{tis_name}' page {}",
                tile.pvrz_page
            ))
        })?;
        // GameData stores names lowercased without extension — strip
        // the `.PVRZ` suffix from the IESDP-style filename and lowercase.
        let lookup_name = pvrz_filename
            .strip_suffix(".PVRZ")
            .unwrap_or(&pvrz_filename)
            .to_ascii_lowercase();
        let resource = game_data
            .get_by_name_and_type(&lookup_name, ResourceType::Pvrz)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "PVRZ resource {pvrz_filename} not found in GameData"
                ))
            })?;
        let ds = resource.datasource.as_ref().ok_or_else(|| {
            io::Error::other(format!("PVRZ resource {pvrz_filename} has no datasource"))
        })?;
        let pvrz = PvrzImporter { name: &lookup_name }.import(ds)?;
        e.insert(pvrz);
    }
    Ok(cache.get(&tile.pvrz_page).expect("just inserted"))
}

/// Copy a 64×64 rectangle out of a decoded PVRZ page into a tile slot.
/// Coordinates that fall outside the page are clamped — matches the
/// out-of-bounds behaviour used by [`crate::imported_resource::bam`]
/// and [`crate::imported_resource::image`] for MOS V2.
fn copy_pvrz_tile(tile: &TisPvrzTile, source_pvrz: &Pvrz, dst: &mut [u8]) {
    let tex_w = source_pvrz.header.width;
    let tex_h = source_pvrz.header.height;
    let mut w = TILE_DIMENSION;
    let mut h = TILE_DIMENSION;
    if tile.source_x + w > tex_w {
        log::warn!(
            "TIS PVRZ tile out of bounds: src_x={} w={} texture_w={}",
            tile.source_x,
            w,
            tex_w
        );
        w = tex_w.saturating_sub(tile.source_x);
    }
    if tile.source_y + h > tex_h {
        log::warn!(
            "TIS PVRZ tile out of bounds: src_y={} h={} texture_h={}",
            tile.source_y,
            h,
            tex_h
        );
        h = tex_h.saturating_sub(tile.source_y);
    }
    if w == 0 || h == 0 {
        return;
    }

    let source_raw = source_pvrz.image.as_raw();
    for row in 0..h {
        let src_row = tile.source_y + row;
        let src_start = ((src_row * tex_w + tile.source_x) * 4) as usize;
        let src_end = src_start + (w * 4) as usize;
        let dst_start = (row * TILE_DIMENSION * 4) as usize;
        let dst_end = dst_start + (w * 4) as usize;
        dst[dst_start..dst_end].copy_from_slice(&source_raw[src_start..src_end]);
    }
}

/// Read the primary overlay width from `<name>.wed` if available.
/// Errors (missing WED, malformed file, …) silently fall back to the
/// caller's default, matching NearInfinity's behaviour.
fn expected_columns_from_wed(name: &str, game_data: &GameData) -> Option<u32> {
    let resource = game_data.get_by_name_and_type(name, ResourceType::Wed)?;
    let ds = resource.datasource.as_ref()?;
    let wed = WedImporter { name }.import(ds).ok()?;
    let primary = wed.overlays.first()?;
    let width = primary.width as u32;
    if width == 0 { None } else { Some(width) }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use infinitier_common::Game;
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};
    use infinitier_tis_resource::TisImporter;

    use crate::game::{DataOrigin, GameResource};

    use super::*;

    fn make_game_data_from_dir(dir: &Path) -> GameData {
        let mut resources = vec![];
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let real_path = entry.path();
            if !real_path.is_file() {
                continue;
            }
            let Some(extension) = real_path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let Some(r#type) = ResourceType::from_extension(extension) else {
                continue;
            };
            let name = real_path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_ascii_lowercase();
            let file_size = real_path.metadata().unwrap().len();
            resources.push(GameResource {
                game_type: Game::Bg2,
                name,
                r#type,
                file_size: Some(file_size),
                datasource: Some(DataSource::new(real_path.as_path())),
                data_origin: DataOrigin::Missing,
                imported: None,
            });
        }
        GameData::new(
            resources,
            Game::Bg2,
            infinitier_fs::CaseInsensitiveFS::empty(),
        )
    }

    fn import_tis(path: &Path) -> Tis {
        let ds = DataSource::new(path);
        TisImporter { name: "test" }.import(&ds).unwrap()
    }

    #[test]
    fn test_load_palette_single_tile_no_wed() {
        // FIRE01: 1 tile, no WED in the test corpus → default columns
        // falls back to `min(tile_count, 5) = 1`.
        let game_data = GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());
        let tis = import_tis(&get_assets_path().join("TIS/Palette/FIRE01.tis"));
        let imported = ImportedTis::load(tis, &game_data, "fire01").unwrap();

        assert_eq!(imported.tile_dimension, 64);
        assert_eq!(imported.tile_count, 1);
        assert_eq!(imported.variant, TisType::Palette);
        assert_eq!(imported.expected_columns, 1);
        assert_eq!(imported.tile_pixels.len(), 64 * 64 * 4);
    }

    #[test]
    fn test_load_palette_multi_tile_caps_default_to_five() {
        // HOTCOALR: 6 tiles, no WED → default columns = min(6, 5) = 5.
        let game_data = GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());
        let tis = import_tis(&get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        let imported = ImportedTis::load(tis, &game_data, "hotcoalr").unwrap();

        assert_eq!(imported.tile_count, 6);
        assert_eq!(imported.expected_columns, 5);
        assert_eq!(imported.tile_pixels.len(), 6 * 64 * 64 * 4);
    }

    #[test]
    fn test_compose_matches_reference_png() {
        // 6×1 layout of the HOTCOALR tileset must reproduce the
        // 384×64 reference PNG byte-for-byte.
        let game_data = GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());
        let tis = import_tis(&get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        let imported = ImportedTis::load(tis, &game_data, "hotcoalr").unwrap();
        let composed = imported.compose(6);
        assert_eq!(composed.width(), 384);
        assert_eq!(composed.height(), 64);
        assert_images_are_equal(
            &image::open(get_assets_path().join("TIS/Palette/HOTCOALR.png")).unwrap(),
            &composed.into(),
            None,
        );
    }

    #[test]
    fn test_load_pvrz_missing_pages_errors() {
        // PVRZ TIS with no PVRZ assets registered → load must surface
        // a "PVRZ resource X not found" error rather than silently
        // producing a black tileset.
        let game_data = GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());
        let tis = import_tis(&get_assets_path().join("TIS/Pvrz/AR0107.tis"));
        let err = ImportedTis::load(tis, &game_data, "ar0107").unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "expected missing-PVRZ error, got: {err}"
        );
    }

    #[test]
    fn test_compose_pads_short_last_row() {
        // 6 tiles in a 4-column grid → 2 rows, the second row half-full.
        // The padded slots must stay fully transparent.
        let game_data = GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());
        let tis = import_tis(&get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        let imported = ImportedTis::load(tis, &game_data, "hotcoalr").unwrap();
        let composed = imported.compose(4);
        assert_eq!(composed.width(), 256);
        assert_eq!(composed.height(), 128);
        // (col=2 of row=1) → (128, 64) is in the padded region.
        assert_eq!(composed.get_pixel(128, 64).0, [0, 0, 0, 0]);
    }

    /// Build a `GameData` that registers the TIS + a synthetic WED for
    /// the same name, so the WED-lookup path produces a non-default
    /// `expected_columns`. Returns the registered TIS resource name.
    fn register_palette_tis_with_wed(asset_dir: &Path, expected_width: u16) -> GameData {
        use infinitier_wed_resource::{ResourceReference, Wed, WedExporter, WedOverlay};

        let tis_path = asset_dir.join("HOTCOALR.tis");
        let tile_count = 6u16; // matches HOTCOALR's tile_count
        let cells =
            (expected_width as usize) * (tile_count as usize).div_ceil(expected_width as usize);
        let primary = WedOverlay {
            width: expected_width,
            height: (tile_count.div_ceil(expected_width)),
            name: ResourceReference {
                name: "hotcoalr".to_string(),
                r#type: ResourceType::Tis,
            },
            unique_tiles_count: 0,
            movement_type: 0,
            tilemap: (0..cells)
                .map(|i| infinitier_wed_resource::WedTilemapEntry {
                    start_index_in_lookup: i as u16,
                    count_in_lookup: 1,
                    secondary_tile_index: -1,
                    overlay_mask: 0,
                    unknown: [0; 3],
                })
                .collect(),
            tile_index_lookup: (0..cells as u16).collect(),
        };
        let wed = Wed {
            overlays: vec![primary],
            doors: vec![],
            wall_polygons: vec![],
            wall_groups: vec![],
            wall_polygon_indexes: vec![],
            verticles: vec![],
            door_tile_cells: vec![],
        };
        let mut wed_bytes: Vec<u8> = Vec::new();
        WedExporter.export(&wed, &mut wed_bytes).unwrap();

        let resources = vec![
            GameResource {
                game_type: Game::Bg2,
                name: "hotcoalr".to_string(),
                r#type: ResourceType::Tis,
                file_size: Some(std::fs::metadata(&tis_path).unwrap().len()),
                datasource: Some(DataSource::new(tis_path.as_path())),
                data_origin: DataOrigin::Missing,
                imported: None,
            },
            GameResource {
                game_type: Game::Bg2,
                name: "hotcoalr".to_string(),
                r#type: ResourceType::Wed,
                file_size: Some(wed_bytes.len() as u64),
                datasource: Some(DataSource::new(wed_bytes)),
                data_origin: DataOrigin::Missing,
                imported: None,
            },
        ];
        GameData::new(
            resources,
            Game::Bg2,
            infinitier_fs::CaseInsensitiveFS::empty(),
        )
    }

    #[test]
    fn test_expected_columns_from_wed_overrides_default() {
        // With a WED registered that says width=3, the importer picks 3
        // instead of the no-WED fallback (min(tile_count, 5) = 5).
        let asset_dir = get_assets_path().join("TIS/Palette");
        let game_data = register_palette_tis_with_wed(&asset_dir, 3);
        let tis = import_tis(&asset_dir.join("HOTCOALR.tis"));
        let imported = ImportedTis::load(tis, &game_data, "hotcoalr").unwrap();
        assert_eq!(imported.expected_columns, 3);
    }

    #[test]
    fn test_load_pvrz_ar0015_matches_reference_png() {
        // AR0015 (BG2:EE) is a PVRZ-based TIS with 352 tiles laid out
        // as a 22 × 16 grid (per its WED). 162 of those tiles use the
        // `-1` "no source" sentinel — NearInfinity and the vanilla
        // engine both paint those as opaque black, *not* transparent
        // (verified empirically against the reference PNG).
        //
        // This test composites the imported tileset at the columns
        // value derived from the WED (which the importer picks
        // automatically) and asserts byte-for-byte equality with the
        // PNG NearInfinity exports.
        let asset_dir = get_assets_path().join("TIS/Pvrz/AR0015");
        let game_data = make_game_data_from_dir(&asset_dir);

        // Sanity: GameData picked up the TIS, WED, and PVRZ page.
        assert!(
            game_data
                .get_by_name_and_type("ar0015", ResourceType::Tis)
                .is_some(),
            "TIS resource must be registered"
        );
        assert!(
            game_data
                .get_by_name_and_type("ar0015", ResourceType::Wed)
                .is_some(),
            "WED resource must be registered (for expected_columns)"
        );
        assert!(
            game_data
                .get_by_name_and_type("a001500", ResourceType::Pvrz)
                .is_some(),
            "page 0 PVRZ must be registered as a001500.pvrz"
        );

        let tis = import_tis(&asset_dir.join("AR0015.tis"));
        let imported = ImportedTis::load(tis, &game_data, "ar0015").unwrap();

        // WED-driven layout: 22 columns × 16 rows = 352 tiles.
        assert_eq!(imported.variant, TisType::Pvrz);
        assert_eq!(imported.tile_count, 352);
        assert_eq!(imported.expected_columns, 22, "WED says width=22");

        let composed = imported.compose(imported.expected_columns);
        assert_eq!(composed.width(), 22 * 64);
        assert_eq!(composed.height(), 16 * 64);

        // Spot-check a known-blank tile (index 0) — must be opaque
        // black, not transparent. Catches regressions of the original
        // "blank = transparent" bug specifically.
        assert_eq!(
            composed.get_pixel(0, 0).0,
            [0, 0, 0, 255],
            "tile 0 is `-1` in the source — must render as opaque black"
        );

        // Full image comparison against the reference PNG exported
        // by NearInfinity. Any drift from byte-exact equality means
        // either the PVRZ decode, the WED-derived layout, or the
        // blank-tile policy has regressed.
        let reference = image::open(asset_dir.join("AR0015.png")).unwrap();
        assert_images_are_equal(&reference, &composed.into(), None);
    }
}
