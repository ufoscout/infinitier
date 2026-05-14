use std::io::{BufRead, Seek};
use std::time::Duration;

use image::{ImageBuffer, Rgba};
use infinitier_datasource::{ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{Type, common::Rgb};

#[derive(Debug, PartialEq, Eq)]
pub struct BamV1 {
    /// The type of the file
    pub r#type: Type,
    /// The frames of the image
    pub frames: Vec<BamV1Frame>,
    /// The colors palette
    pub palette: Vec<Rgb>,
    /// The image cycles
    pub cycles: Vec<BamV1Cycle>,
    /// The index of the RLE compressed color in the palette
    pub rle_compressed_color_index: u8,
}

impl BamV1 {
    /// Default playback rate used by the Infinity Engine for BAM
    /// animations. The format itself does not encode timing — NearInfinity
    /// hardcodes the same value (`ANIM_DELAY = 1000 / 15`) for all cycles
    /// in `BamResource.java`.
    pub const DEFAULT_FPS: u32 = 15;

    /// Default per-frame duration corresponding to [`Self::DEFAULT_FPS`].
    pub const DEFAULT_FRAME_DURATION: Duration =
        Duration::from_nanos(1_000_000_000 / Self::DEFAULT_FPS as u64);

    /// Render frame `frame_idx` of cycle `cycle_idx` into the cycle's
    /// shared canvas, positioned so its anchor lines up with the shared
    /// origin. `frame_idx` indexes into the cycle's `frame_indices`
    /// (i.e. it's a frame-in-cycle, not a global frame index).
    /// Returns `None` when either index is out of bounds.
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
                let idx = (fy as u32 * frame.width + fx as u32) as usize;
                let p = &self.palette[frame.pixel_palette_indexes[idx] as usize];
                canvas.put_pixel(dx as u32, dy as u32, Rgba([p.r, p.g, p.b, p.alpha]));
            }
        }

        Some(canvas)
    }
}

/// Shared canvas rectangle for one or more BAM frames. `(x, y)` is the
/// top-left corner relative to the anchor at the origin; `(width, height)`
/// is the bounding-box size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A BAM V1 file importer
pub struct BamV1Parser;

impl BamV1Parser {
    /// Imports a BAM V1 file
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<BamV1> {
        let signature = reader.read_string(8)?;
        let expected_type = Type::BamV1;

        if !signature.eq(expected_type.signature()) {
            error!("Not a BAM V1 file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let frames_count = reader.read_u16()? as usize;
        let cycles_count = reader.read_u8()? as usize;
        let rle_compressed_color_index = reader.read_u8()?;

        let frames_offset = reader.read_u32()? as u64;
        let palette_offset = reader.read_u32()? as u64;
        let lookup_offset = reader.read_u32()? as u64;

        // Initializing palette
        let palette = {
            // Find the nearest section offset after palette_offset to determine palette size,
            // regardless of which section (frames or lookup table) comes after it.
            let file_end = reader.seek(std::io::SeekFrom::End(0))?;
            let next_offset = [frames_offset, lookup_offset, file_end]
                .into_iter()
                .filter(|&o| o > palette_offset)
                .min()
                .unwrap_or(file_end);
            let palette_entries = 256.min((next_offset - palette_offset) / 4) as usize;
            let mut palette = Vec::with_capacity(palette_entries);
            reader.set_position(palette_offset)?;

            let mut transparency_index = 0;

            for i in 0..palette_entries {
                let b = reader.read_u8()?;
                let g = reader.read_u8()?;
                let r = reader.read_u8()?;
                let alpha = match reader.read_u8()? {
                    0 => 255, // BAM in EE supports alpha, but for backwards compatibility an alpha of 0 is still 255
                    x => x, // Alpha values of 01h .. FFh indicate transparency ranging from almost completely transparent to fully opaque. Full transparency can be realized by using palette index 0.
                };

                // The transparency index is set to the first occurence of RGB(0,255,0).
                // If RGB(0,255,0) does not exist in the palette then transparency index is set to 0
                if transparency_index == 0 && r == 0 && g == 255 && b == 0 {
                    transparency_index = i;
                }

                palette.push(Rgb { r, g, b, alpha });
            }

            if transparency_index < palette.len() {
                let _ = std::mem::replace(
                    &mut palette[transparency_index],
                    Rgb {
                        r: 0,
                        g: 255,
                        b: 0,
                        alpha: 0,
                    },
                );
            }

            palette
        };

        // initializing frames
        let frames = {
            reader.set_position(frames_offset)?;
            let mut frames = Vec::with_capacity(frames_count);
            for _ in 0..frames_count {
                let width = reader.read_u16()? as u32;
                let height = reader.read_u16()? as u32;
                let center_x = reader.read_i16()? as i32;
                let center_y = reader.read_i16()? as i32;
                let data_bits = reader.read_u32()?;
                let data_offset = (data_bits & 0x7fffffff) as u64;
                let compressed = (data_bits & 0x80000000) == 0;

                let size = (width * height) as usize;
                let position = reader.position()?;

                let mut pixel_palette_indexes = Vec::with_capacity(size);
                reader.set_position(data_offset)?;
                while pixel_palette_indexes.len() < size {
                    let pixel_index = reader.read_u8()?;

                    if compressed && (pixel_index == rle_compressed_color_index) {
                        // Some BAM files end with an RLE token but no count byte; treat as count=0
                        let pixels_count = reader.read_u8().unwrap_or(0);
                        for _ in 0..=pixels_count {
                            pixel_palette_indexes.push(pixel_index);
                        }
                    } else {
                        pixel_palette_indexes.push(pixel_index);
                    }
                }

                reader.set_position(position)?;

                frames.push(BamV1Frame {
                    width,
                    height,
                    center_x,
                    center_y,
                    pixel_palette_indexes,
                });
            }

            frames
        };

        // initializing cycles
        let cycles = {
            let mut cycles = Vec::with_capacity(cycles_count);
            for _ in 0..cycles_count {
                // number of frame indices in this cycle
                let indices_count = reader.read_u16()? as usize;
                // Index into frame lookup table of first frame in this cycle
                let lookup_table_index = reader.read_u16()? as u64;

                let position = reader.position()?;

                // list of frame indices used in this cycle
                let mut frame_indices = Vec::with_capacity(indices_count);
                reader.set_position(lookup_offset + (2 * lookup_table_index))?;
                for _ in 0..indices_count {
                    let frame_index = reader.read_u16()?;
                    frame_indices.push(frame_index as usize);
                }

                cycles.push(BamV1Cycle::new(frame_indices, &frames));

                reader.set_position(position)?;
            }
            cycles
        };

        debug!(
            "Loaded BAM V1: {} frames, {} cycles",
            frames.len(),
            cycles.len()
        );
        Ok(BamV1 {
            r#type: expected_type,
            frames,
            cycles,
            palette,
            rle_compressed_color_index,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV1Cycle {
    pub frame_indices: Vec<usize>,
    /// Pre-computed bounding box that fits every frame in this cycle
    /// when each frame is positioned by its anchor `(center_x, center_y)`.
    /// Drawing each frame into a canvas of `shared_rect.{width, height}`
    /// at `(-shared_rect.x - center_x, -shared_rect.y - center_y)` keeps
    /// the anchor pinned on screen — see [`BamV1::render_frame_centered`].
    pub shared_rect: SharedRect,
}

impl BamV1Cycle {
    /// Build a cycle from its frame-index list, computing
    /// [`Self::shared_rect`] from the referenced frames. Mirrors
    /// NearInfinity's `BamControl.calculateSharedBamSize` in SHARED mode
    /// with `sharedPerCycle = true` — the rectangle is the union of every
    /// frame's footprint when each is anchored at the origin.
    pub fn new(frame_indices: Vec<usize>, frames: &[BamV1Frame]) -> Self {
        let mut x1 = i32::MAX;
        let mut x2 = i32::MIN;
        let mut y1 = i32::MAX;
        let mut y2 = i32::MIN;

        for &frame_idx in &frame_indices {
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

        Self {
            frame_indices,
            shared_rect: SharedRect {
                x: x1,
                y: y1,
                width: (x2 - x1 + 1) as u32,
                height: (y2 - y1 + 1) as u32,
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV1Frame {
    pub width: u32,
    pub height: u32,
    pub center_x: i32,
    pub center_y: i32,
    /// The indexes of the pixels in the palette
    pub pixel_palette_indexes: Vec<u8>,
}

impl BamV1Frame {
    /// Exports the frame to an image.
    /// The image type is determined by the file extension.
    pub fn to_image(&self, palette: &[Rgb]) -> image::ImageResult<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        Ok(ImageBuffer::from_fn(self.width, self.height, |x, y| {
            let idx = (y * self.width + x) as usize;
            let p = &palette[self.pixel_palette_indexes[idx] as usize];
            Rgba([p.r, p.g, p.b, p.alpha])
        }))
    }
}

#[cfg(test)]
mod tests {

    use infinitier_datasource::DataSource;

    use super::*;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    #[test]
    fn test_parse_bam_v1_should_fail_if_wrong_signature() {
        let data = DataSource::new(get_assets_path().join("BAM_V1/01/1chan03B_compressed.BAM"));

        let mut reader = data.reader().unwrap();
        let res = BamV1Parser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_bam_v1_01() {
        let data = DataSource::new(get_assets_path().join("BAM_V1/01/1chan03B_decompressed.BAM"));

        let mut reader = data.reader().unwrap();
        let bam = BamV1Parser::import(&mut reader).unwrap();

        assert_eq!(bam.r#type, Type::BamV1);

        assert_eq!(bam.rle_compressed_color_index, 0);

        assert_eq!(bam.cycles.len(), 1);
        assert_eq!(bam.cycles[0].frame_indices, vec![0, 0]);
        // Single frame 50×60 anchored at (25, 30): x ∈ [-25, 25], y ∈ [-30, 30].
        assert_eq!(
            bam.cycles[0].shared_rect,
            SharedRect {
                x: -25,
                y: -30,
                width: 51,
                height: 61,
            }
        );

        assert_eq!(bam.frames.len(), 1);
        assert_eq!(bam.frames[0].width, 50);
        assert_eq!(bam.frames[0].height, 60);
        assert_eq!(bam.frames[0].center_x, 25);
        assert_eq!(bam.frames[0].center_y, 30);
        assert_eq!(bam.frames[0].pixel_palette_indexes.len(), 50 * 60);

        // Assert that the image is the same as the reference
        {
            let image = bam.frames[0].to_image(&bam.palette).unwrap();

            assert_images_are_equal(
                &image::open(get_assets_path().join("BAM_V1/01/1chan03B00000.PNG")).unwrap(),
                &image.into(),
            );
        }
    }

    #[test]
    fn test_parse_bam_v1_02() {
        let data = DataSource::new(get_assets_path().join("BAM_V1/02/SPHEART_decompressed.BAM"));

        let mut reader = data.reader().unwrap();
        let bam = BamV1Parser::import(&mut reader).unwrap();

        assert_eq!(bam.r#type, Type::BamV1);

        assert_eq!(bam.rle_compressed_color_index, 0);

        assert_eq!(bam.cycles.len(), 1);
        assert_eq!(
            bam.cycles[0].frame_indices,
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
        );

        // 15 frames with assorted sizes/centers, so exact shared-rect
        // numbers are tedious to verify by hand. Instead check the
        // invariant: every frame's anchored footprint must fit inside
        // the shared rect.
        let rect = bam.cycles[0].shared_rect;
        for &idx in &bam.cycles[0].frame_indices {
            let f = &bam.frames[idx];
            assert!(-f.center_x >= rect.x);
            assert!(-f.center_y >= rect.y);
            assert!(f.width as i32 - f.center_x <= rect.x + rect.width as i32 - 1);
            assert!(f.height as i32 - f.center_y <= rect.y + rect.height as i32 - 1);
        }

        assert_eq!(bam.frames.len(), 15);

        for (i, frame) in bam.frames.iter().enumerate() {
            assert!(frame.center_x > 0);
            assert!(frame.center_x < frame.width as i32);
            assert!(frame.center_y > 0);
            assert!(frame.center_y < frame.height as i32);
            assert_eq!(
                frame.pixel_palette_indexes.len(),
                (frame.width * frame.height) as usize
            );

            // Assert that the image is the same as the reference
            {
                let image = frame.to_image(&bam.palette).unwrap();

                assert_images_are_equal(
                    &image::open(get_assets_path().join(format!("BAM_V1/02/SPHEART000{i:02}.PNG")))
                        .unwrap(),
                    &image.into(),
                );
            }
        }
    }

    #[test]
    fn test_parse_bam_v1_03() {
        let data = DataSource::new(get_assets_path().join("BAM_V1/03/SPWI524D_decompressed.BAM"));

        let mut reader = data.reader().unwrap();
        let bam = BamV1Parser::import(&mut reader).unwrap();

        assert_eq!(bam.r#type, Type::BamV1);

        assert_eq!(bam.rle_compressed_color_index, 0);

        assert_eq!(bam.cycles.len(), 1);
        assert_eq!(bam.cycles[0].frame_indices, vec![0]);
        // Single frame 13×13 anchored at (0, 13): x ∈ [0, 13], y ∈ [-13, 0].
        assert_eq!(
            bam.cycles[0].shared_rect,
            SharedRect {
                x: 0,
                y: -13,
                width: 14,
                height: 14,
            }
        );

        assert_eq!(bam.frames.len(), 1);
        assert_eq!(bam.frames[0].width, 13);
        assert_eq!(bam.frames[0].height, 13);
        assert_eq!(bam.frames[0].center_x, 0);
        assert_eq!(bam.frames[0].center_y, 13);
        assert_eq!(bam.frames[0].pixel_palette_indexes.len(), 13 * 13);

        // Assert that the image is the same as the reference
        {
            let image = bam.frames[0].to_image(&bam.palette).unwrap();

            assert_images_are_equal(
                &image::open(get_assets_path().join("BAM_V1/03/SPWI524D00000.PNG")).unwrap(),
                &image.into(),
            );
        }
    }

    /// Build a frame for synthetic shared-rect tests. Pixel data is
    /// unused by the calculation, so we keep it minimal.
    fn synth_frame(width: u32, height: u32, center_x: i32, center_y: i32) -> BamV1Frame {
        BamV1Frame {
            width,
            height,
            center_x,
            center_y,
            pixel_palette_indexes: vec![0; (width * height) as usize],
        }
    }

    #[test]
    fn test_cycle_shared_rect_empty_cycle() {
        // No frame indices → degenerate 1×1 rect at origin.
        let cycle = BamV1Cycle::new(vec![], &[]);
        assert_eq!(
            cycle.shared_rect,
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
        let cycle = BamV1Cycle::new(vec![0], &frames);
        assert_eq!(
            cycle.shared_rect,
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
        let cycle = BamV1Cycle::new(vec![0], &frames);
        assert_eq!(
            cycle.shared_rect,
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
        let cycle = BamV1Cycle::new(vec![0, 1, 2], &frames);
        assert_eq!(
            cycle.shared_rect,
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
        let cycle = BamV1Cycle::new(vec![0, 5], &frames);
        assert_eq!(
            cycle.shared_rect,
            SharedRect {
                x: 0,
                y: 0,
                width: 11,
                height: 9,
            }
        );
    }
}
