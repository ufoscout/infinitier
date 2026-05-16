use std::collections::HashMap;
use std::io;

use image::{ImageBuffer, Rgba};
use infinitier_bam_importer::{Bam, BamV1, BamV2, BamV2DataBlock, SharedRect, Type};
use infinitier_common::ResourceType;
use infinitier_datasource::Importer;
use infinitier_pvr_importer::{PvrzHeader, PvrzImporter};

use crate::game::GameData;

/// A preloaded BAM file. All pixel data is decoded eagerly so the UI /
/// game loop only needs to upload textures and apply per-cycle anchoring.
#[derive(Debug)]
pub struct ImportedBam {
    pub r#type: ResourceType,
    /// The on-disk BAM variant.
    pub bam_type: Type,
    pub frames: Vec<BamFrame>,
    pub cycles: Vec<BamCycle>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamFrame {
    /// Frame width in pixels — matches `image.width()`.
    pub width: u32,
    /// Frame height in pixels — matches `image.height()`.
    pub height: u32,
    /// X anchor used to position the frame inside a cycle's shared canvas.
    pub center_x: i32,
    /// Y anchor used to position the frame inside a cycle's shared canvas.
    pub center_y: i32,
    /// Pre-decoded RGBA pixels for the frame at its own (width × height).
    pub image: ImageBuffer<Rgba<u8>, Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamCycle {
    /// Indices into [`ImportedBam::frames`] that make up this animation.
    pub frame_indices: Vec<usize>,
    /// Bounding box that fits every frame in the cycle when each is
    /// positioned by its anchor — see [`infinitier_bam_importer::SharedRect`].
    pub shared_rect: SharedRect,
}

impl ImportedBam {
    /// Decode every frame ahead of time. For V1/BAMC this unpacks the
    /// palette indices; for V2 this resolves each frame's data blocks
    /// against the PVRZ pages found in `game_data` and composites them.
    pub fn load(bam: Bam, game_data: &GameData) -> io::Result<Self> {
        match bam {
            Bam::V1(bam) => load_v1(bam),
            Bam::V2(bam) => load_v2(bam, game_data),
        }
    }

    /// Composite frame `frame_idx` of cycle `cycle_idx` into the cycle's
    /// shared canvas, positioned so the frame's anchor lines up with the
    /// shared origin. `frame_idx` indexes into the cycle's
    /// `frame_indices`. Returns `None` when either index is out of bounds.
    pub fn render_frame_centered(
        &self,
        cycle_idx: usize,
        frame_idx: usize,
    ) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let cycle = self.cycles.get(cycle_idx)?;
        let global_frame_idx = *cycle.frame_indices.get(frame_idx)?;
        let frame = self.frames.get(global_frame_idx)?;
        let shared = cycle.shared_rect;

        let mut canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(shared.width, shared.height, Rgba([0, 0, 0, 0]));

        let dest_x = -shared.x - frame.center_x;
        let dest_y = -shared.y - frame.center_y;

        for fy in 0..frame.height as i32 {
            let dy = dest_y + fy;
            if dy < 0 || dy as u32 >= shared.height {
                continue;
            }
            for fx in 0..frame.width as i32 {
                let dx = dest_x + fx;
                if dx < 0 || dx as u32 >= shared.width {
                    continue;
                }
                let p = frame.image.get_pixel(fx as u32, fy as u32);
                canvas.put_pixel(dx as u32, dy as u32, *p);
            }
        }

        Some(canvas)
    }
}

fn load_v1(bam: BamV1) -> io::Result<ImportedBam> {
    let frames = bam
        .frames
        .iter()
        .map(|frame| -> io::Result<BamFrame> {
            // BamV1Frame::to_image never actually fails — it always
            // returns Ok — but the signature is fallible so we forward.
            let image = frame.to_image(&bam.palette).map_err(io::Error::other)?;
            Ok(BamFrame {
                width: frame.width,
                height: frame.height,
                center_x: frame.center_x,
                center_y: frame.center_y,
                image,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let cycles = bam
        .cycles
        .into_iter()
        .map(|c| {
            let shared_rect = cycle_shared_rect(&c.frame_indices, &frames);
            BamCycle {
                frame_indices: c.frame_indices,
                shared_rect,
            }
        })
        .collect();

    Ok(ImportedBam {
        r#type: ResourceType::Bam,
        bam_type: bam.r#type,
        frames,
        cycles,
    })
}

fn load_v2(bam: BamV2, game_data: &GameData) -> io::Result<ImportedBam> {
    // PVRZ pages are shared across many BAM frames; decode each page
    // (DXT1/DXT5 decompression + zlib) at most once.
    let mut pvrz_cache: HashMap<u32, (PvrzHeader, ImageBuffer<Rgba<u8>, Vec<u8>>)> = HashMap::new();

    let frames = bam
        .frames
        .iter()
        .map(|frame| -> io::Result<BamFrame> {
            let blocks_end = frame
                .data_blocks_start_index
                .checked_add(frame.data_blocks_count)
                .ok_or_else(|| io::Error::other("data_blocks range overflow"))?;
            let blocks = bam
                .data_blocks
                .get(frame.data_blocks_start_index..blocks_end)
                .ok_or_else(|| {
                    io::Error::other(format!(
                        "data_blocks range {}..{} out of bounds (have {})",
                        frame.data_blocks_start_index,
                        blocks_end,
                        bam.data_blocks.len(),
                    ))
                })?;

            let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(frame.width, frame.height);
            for block in blocks {
                let (header, source) = get_or_load_pvrz(&mut pvrz_cache, block, game_data)?;
                paste_block(block, header, source, frame.width, image.as_mut());
            }

            Ok(BamFrame {
                width: frame.width,
                height: frame.height,
                center_x: frame.center_x,
                center_y: frame.center_y,
                image,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let cycles = bam
        .cycles
        .iter()
        .map(|c| {
            let frame_indices: Vec<usize> =
                (c.frame_start_index..c.frame_start_index + c.frames_count).collect();
            let shared_rect = cycle_shared_rect(&frame_indices, &frames);
            BamCycle {
                frame_indices,
                shared_rect,
            }
        })
        .collect();

    Ok(ImportedBam {
        r#type: ResourceType::Bam,
        bam_type: bam.r#type,
        frames,
        cycles,
    })
}

/// Decode the PVRZ page referenced by `block`, caching the result so
/// later frames sharing the same atlas don't re-decode.
fn get_or_load_pvrz<'cache>(
    cache: &'cache mut HashMap<u32, (PvrzHeader, ImageBuffer<Rgba<u8>, Vec<u8>>)>,
    block: &BamV2DataBlock,
    game_data: &GameData,
) -> io::Result<(&'cache PvrzHeader, &'cache ImageBuffer<Rgba<u8>, Vec<u8>>)> {
    if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(block.pvrz_page) {
        // GameData stores names without extension, lowercased.
        let name = format!("mos{:04}", block.pvrz_page);
        let resource = game_data
            .get_by_name_and_type(&name, ResourceType::Pvrz)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "PVRZ resource {} not found in GameData",
                    block.pvrz_name(),
                ))
            })?;
        let ds = resource
            .datasource
            .as_ref()
            .ok_or_else(|| io::Error::other(format!("PVRZ resource {} has no datasource", name)))?;
        let importer = PvrzImporter { name: &name };
        let header = importer.import(ds)?;
        let image = PvrzImporter::to_image(&header, ds).map_err(io::Error::other)?;
        e.insert((header, image));
    }
    let entry = cache.get(&block.pvrz_page).expect("just inserted");
    Ok((&entry.0, &entry.1))
}

/// Copy one BAM V2 data block out of a decoded PVRZ page and into the
/// frame's target buffer. Mirrors `BamV2::frame_to_image` but operates
/// on an already-decoded source image — same out-of-bounds clamping
/// behaviour so malformed files don't crash.
fn paste_block(
    block: &BamV2DataBlock,
    source_header: &PvrzHeader,
    source: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    target_width: u32,
    target_buffer: &mut [u8],
) {
    let mut w = block.width;
    let mut h = block.height;
    if block.source_x_coordinate + w > source_header.width {
        log::warn!(
            "PVRZ {} texture width out of bounds: src_x={} w={} texture_w={}",
            block.pvrz_name(),
            block.source_x_coordinate,
            w,
            source_header.width
        );
        w = source_header
            .width
            .saturating_sub(block.source_x_coordinate);
    }
    if block.source_y_coordinate + h > source_header.height {
        log::warn!(
            "PVRZ {} texture height out of bounds: src_y={} h={} texture_h={}",
            block.pvrz_name(),
            block.source_y_coordinate,
            h,
            source_header.height
        );
        h = source_header
            .height
            .saturating_sub(block.source_y_coordinate);
    }
    if w == 0 || h == 0 {
        return;
    }

    let source_raw = source.as_raw();
    for row in 0..h {
        let block_source_row = block.source_y_coordinate + row;
        let block_destination_row = block.target_y_coordinate + row;

        let source_start =
            ((block_source_row * source_header.width + block.source_x_coordinate) * 4) as usize;
        let source_end = source_start + (w * 4) as usize;

        let target_start =
            ((block_destination_row * target_width + block.target_x_coordinate) * 4) as usize;
        let target_end = target_start + (w * 4) as usize;

        target_buffer[target_start..target_end]
            .copy_from_slice(&source_raw[source_start..source_end]);
    }
}

/// Compute the per-cycle bounding box that fits every frame in the
/// cycle when each is positioned by its anchor `(center_x, center_y)`.
/// Mirrors NearInfinity's `BamControl.calculateSharedBamSize` in SHARED
/// mode with `sharedPerCycle = true` — the rectangle is the union of
/// every frame's footprint when each is anchored at the origin.
fn cycle_shared_rect(frame_indices: &[usize], frames: &[BamFrame]) -> SharedRect {
    let mut x1 = i32::MAX;
    let mut x2 = i32::MIN;
    let mut y1 = i32::MAX;
    let mut y2 = i32::MIN;

    for &frame_idx in frame_indices {
        if let Some(frame) = frames.get(frame_idx) {
            let w = frame.width as i32;
            let h = frame.height as i32;
            x1 = x1.min(-frame.center_x);
            y1 = y1.min(-frame.center_y);
            x2 = x2.max(w - frame.center_x);
            y2 = y2.max(h - frame.center_y);
        }
    }

    if x1 == i32::MAX {
        x1 = 0;
    }
    if y1 == i32::MAX {
        y1 = 0;
    }
    if x2 == i32::MIN {
        x2 = 0;
    }
    if y2 == i32::MIN {
        y2 = 0;
    }

    SharedRect {
        x: x1,
        y: y1,
        width: (x2 - x1 + 1) as u32,
        height: (y2 - y1 + 1) as u32,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use infinitier_bam_importer::BamImporter;
    use infinitier_common::Game;
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    use crate::game::{DataOrigin, GameResource};

    use super::*;

    /// Build a [`GameData`] populated with every typed resource found in
    /// `dir`. Filenames lose their extension and get lowercased to match
    /// the indexing convention of [`GameData::get_by_name_and_type`].
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
            });
        }
        GameData::new(resources, Game::Bg2)
    }

    fn import_bam(path: &Path) -> Bam {
        let ds = DataSource::new(path);
        let mut reader = ds.reader().unwrap();
        BamImporter::from_reader(&mut reader).unwrap()
    }

    #[test]
    fn test_load_bam_v1() {
        // V1 does not need PVRZ lookup, so an empty GameData is fine.
        let game_data = GameData::new(vec![], Game::Bg2);
        let bam = import_bam(&get_assets_path().join("BAM_V1/01/1chan03B_decompressed.BAM"));
        let imported = ImportedBam::load(bam, &game_data).unwrap();

        assert_eq!(imported.r#type, ResourceType::Bam);
        assert_eq!(imported.bam_type, Type::BamV1);

        assert_eq!(imported.frames.len(), 1);
        assert_eq!(imported.frames[0].width, 50);
        assert_eq!(imported.frames[0].height, 60);
        assert_eq!(imported.frames[0].center_x, 25);
        assert_eq!(imported.frames[0].center_y, 30);
        assert_eq!(imported.frames[0].image.width(), 50);
        assert_eq!(imported.frames[0].image.height(), 60);

        assert_eq!(imported.cycles.len(), 1);
        assert_eq!(imported.cycles[0].frame_indices, vec![0, 0]);
        assert_eq!(
            imported.cycles[0].shared_rect,
            SharedRect {
                x: -25,
                y: -30,
                width: 51,
                height: 61,
            }
        );

        // Pixel data must match the reference PNG.
        assert_images_are_equal(
            &image::open(get_assets_path().join("BAM_V1/01/1chan03B00000.PNG")).unwrap(),
            &imported.frames[0].image.clone().into(),
        );
    }

    #[test]
    fn test_load_bam_v1_compressed() {
        // BAMC is decompressed during import and surfaces as Bam::V1, so
        // the loaded result must be identical to the decompressed copy.
        let game_data = GameData::new(vec![], Game::Bg2);
        let bam = import_bam(&get_assets_path().join("BAM_V1/01/1chan03B_compressed.BAM"));
        let imported = ImportedBam::load(bam, &game_data).unwrap();

        let bam_dec = import_bam(&get_assets_path().join("BAM_V1/01/1chan03B_decompressed.BAM"));
        let imported_dec = ImportedBam::load(bam_dec, &game_data).unwrap();

        assert_eq!(imported.bam_type, Type::BamV1);
        assert_eq!(imported.frames, imported_dec.frames);
        assert_eq!(imported.cycles, imported_dec.cycles);
    }

    #[test]
    fn test_load_bam_v1_multi_frame() {
        let game_data = GameData::new(vec![], Game::Bg2);
        let bam = import_bam(&get_assets_path().join("BAM_V1/02/SPHEART_decompressed.BAM"));
        let imported = ImportedBam::load(bam, &game_data).unwrap();

        assert_eq!(imported.bam_type, Type::BamV1);
        assert_eq!(imported.frames.len(), 15);
        assert_eq!(imported.cycles.len(), 1);
        assert_eq!(
            imported.cycles[0].frame_indices,
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
        );

        // Every per-frame image must match its reference PNG.
        for (i, frame) in imported.frames.iter().enumerate() {
            let reference =
                image::open(get_assets_path().join(format!("BAM_V1/02/SPHEART000{i:02}.PNG")))
                    .unwrap();
            assert_images_are_equal(&reference, &frame.image.clone().into());
        }
    }

    #[test]
    fn test_load_bam_v2() {
        let asset_dir = get_assets_path().join("BAM_V2");
        let game_data = make_game_data_from_dir(&asset_dir);
        let bam = import_bam(&asset_dir.join("1CHELM03.BAM"));
        let imported = ImportedBam::load(bam, &game_data).unwrap();

        assert_eq!(imported.r#type, ResourceType::Bam);
        assert_eq!(imported.bam_type, Type::BamV2);

        assert_eq!(imported.frames.len(), 1);
        assert_eq!(imported.frames[0].width, 128);
        assert_eq!(imported.frames[0].height, 154);
        assert_eq!(imported.frames[0].center_x, 0);
        assert_eq!(imported.frames[0].center_y, 0);
        assert_eq!(imported.frames[0].image.width(), 128);
        assert_eq!(imported.frames[0].image.height(), 154);

        assert_eq!(imported.cycles.len(), 1);
        // V2 cycle: start=0, count=1 → [0]
        assert_eq!(imported.cycles[0].frame_indices, vec![0]);
        // Single 128×154 frame anchored at (0, 0): x ∈ [0, 128], y ∈ [0, 154].
        assert_eq!(
            imported.cycles[0].shared_rect,
            SharedRect {
                x: 0,
                y: 0,
                width: 129,
                height: 155,
            }
        );

        // V2 composition (PVRZ blits) must match the reference PNG.
        assert_images_are_equal(
            &image::open(asset_dir.join("1CHELM0300000.PNG")).unwrap(),
            &imported.frames[0].image.clone().into(),
        );
    }

    #[test]
    fn test_load_bam_v2_missing_pvrz_errors() {
        // Empty GameData → the required `mos0000` PVRZ is unavailable.
        let game_data = GameData::new(vec![], Game::Bg2);
        let bam = import_bam(&get_assets_path().join("BAM_V2/1CHELM03.BAM"));
        let err = ImportedBam::load(bam, &game_data).unwrap_err();
        assert!(
            err.to_string().contains("MOS0000"),
            "expected error about missing PVRZ, got {err}"
        );
    }

    /// Build a frame for synthetic shared-rect tests. Pixel data is
    /// unused by the calculation, so we keep it minimal.
    fn synth_frame(width: u32, height: u32, center_x: i32, center_y: i32) -> BamFrame {
        BamFrame {
            width,
            height,
            center_x,
            center_y,
            image: ImageBuffer::new(width, height),
        }
    }

    #[test]
    fn test_cycle_shared_rect_empty_cycle() {
        // No frame indices → degenerate 1×1 rect at origin.
        assert_eq!(
            cycle_shared_rect(&[], &[]),
            SharedRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }
        );
    }

    #[test]
    fn test_cycle_shared_rect_single_frame_anchor_at_origin() {
        // Frame 10×8 anchored at (0, 0): footprint is [0, 10] × [0, 8].
        let frames = vec![synth_frame(10, 8, 0, 0)];
        assert_eq!(
            cycle_shared_rect(&[0], &frames),
            SharedRect {
                x: 0,
                y: 0,
                width: 11,
                height: 9,
            }
        );
    }

    #[test]
    fn test_cycle_shared_rect_single_frame_anchor_centered() {
        // Frame 20×10 anchored at (10, 5): footprint is [-10, 10] × [-5, 5].
        let frames = vec![synth_frame(20, 10, 10, 5)];
        assert_eq!(
            cycle_shared_rect(&[0], &frames),
            SharedRect {
                x: -10,
                y: -5,
                width: 21,
                height: 11,
            }
        );
    }

    #[test]
    fn test_cycle_shared_rect_multiple_frames_union() {
        // Frame 0: 10×10 anchored at (5, 5) → x ∈ [-5, 5], y ∈ [-5, 5]
        // Frame 1: 4×8  anchored at (0, 0) → x ∈ [0, 4],   y ∈ [0, 8]
        // Frame 2: 6×6  anchored at (6, 6) → x ∈ [-6, 0],  y ∈ [-6, 0]
        // Union: x ∈ [-6, 5], y ∈ [-6, 8] → 12 × 15.
        let frames = vec![
            synth_frame(10, 10, 5, 5),
            synth_frame(4, 8, 0, 0),
            synth_frame(6, 6, 6, 6),
        ];
        assert_eq!(
            cycle_shared_rect(&[0, 1, 2], &frames),
            SharedRect {
                x: -6,
                y: -6,
                width: 12,
                height: 15,
            }
        );
    }

    #[test]
    fn test_cycle_shared_rect_skips_invalid_frame_indices() {
        // Index 5 is out of range and must be ignored, leaving the rect
        // identical to a cycle containing only the valid frame.
        let frames = vec![synth_frame(10, 8, 0, 0)];
        assert_eq!(
            cycle_shared_rect(&[0, 5], &frames),
            SharedRect {
                x: 0,
                y: 0,
                width: 11,
                height: 9,
            }
        );
    }
}
