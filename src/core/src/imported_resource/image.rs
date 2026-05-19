//! Unified image abstraction over BMP / PVRZ / MOS (and, later, TGA /
//! PNG / TIS …). Viewers and game code that just want "RGBA8 pixels
//! plus a label" should work against [`ImportedImage`] instead of each
//! format's bespoke importer output, so adding a new image format is a
//! single new constructor here rather than a fresh viewer.

use std::collections::HashMap;
use std::io;

use image::{ImageBuffer, Rgba};
use infinitier_bmp_resource::{Bmp, BmpCompression};
use infinitier_common::ResourceType;
use infinitier_datasource::Importer;
use infinitier_mos_resource::{Mos, MosV2, MosV2DataBlock, Type as MosType};
use infinitier_pvrz_resource::{PvrDataCompression, Pvrz, PvrzImporter};

use crate::game::GameData;

/// A preloaded image plus the metadata of the format it came from.
/// Pixel data is always RGBA8 regardless of source — callers don't need
/// to dispatch on the source format unless they want to surface
/// format-specific details (e.g. BMP bit depth in an info bar).
#[derive(Debug)]
pub struct ImportedImage {
    /// Decoded RGBA8 pixels at the image's natural dimensions.
    pub image: ImageBuffer<Rgba<u8>, Vec<u8>>,
    /// Format the image was decoded from, with any format-specific
    /// metadata the viewer might want to show.
    pub source: ImageSource,
}

/// Format-specific metadata for an [`ImportedImage`]. Tagged with the
/// original source format so the viewer can show "32 bpp, BI_RGB" for
/// BMP vs "DXT5" for PVRZ without spreading every variant's fields
/// onto every image.
#[derive(Debug)]
pub enum ImageSource {
    Bmp {
        /// Original bit depth as declared in the DIB header
        /// (1, 4, 8, 16, 24, 32).
        bit_count: u16,
        /// Compression scheme as declared in the DIB header.
        compression: BmpCompression,
    },
    Pvr {
        /// The DXT compression variant the source PVR(Z) used.
        pixel_format: PvrDataCompression,
    },
    Mos {
        /// Which MOS wrapper the data came from — V1 (paletted), MOSC
        /// (zlib-wrapped V1), or V2 (PVRZ atlas). Distinguishes the
        /// otherwise-collapsed V1/MOSC pair in the info bar.
        variant: MosType,
    },
}

impl ImportedImage {
    /// Adopt an already-decoded [`Bmp`]. The pixel buffer is moved into
    /// the new wrapper, no allocation.
    pub fn from_bmp(bmp: Bmp) -> Self {
        Self {
            image: bmp.image,
            source: ImageSource::Bmp {
                bit_count: bmp.bit_count,
                compression: bmp.compression,
            },
        }
    }

    /// Adopt an already-decoded [`Pvrz`]. The pixel buffer is moved into
    /// the new wrapper, no allocation.
    pub fn from_pvrz(pvrz: Pvrz) -> Self {
        Self {
            image: pvrz.image,
            source: ImageSource::Pvr {
                pixel_format: pvrz.header.pixel_format,
            },
        }
    }

    /// Adopt an already-decoded [`Mos`]. V1 and MOSC produce their image
    /// from per-block palettes directly; V2 resolves the referenced
    /// `MOS<page:04>.PVRZ` pages from `game_data` and pastes each block
    /// into a fresh RGBA buffer, mirroring `ImportedBam::load` for BAM V2.
    pub fn from_mos(mos: Mos, game_data: &GameData) -> io::Result<Self> {
        match mos {
            Mos::V1(v1) => {
                let variant = v1.r#type;
                Ok(Self {
                    image: v1.to_image(),
                    source: ImageSource::Mos { variant },
                })
            }
            Mos::V2(v2) => Self::from_mos_v2(v2, game_data),
        }
    }

    fn from_mos_v2(v2: MosV2, game_data: &GameData) -> io::Result<Self> {
        let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(v2.width, v2.height);
        let mut pvrz_cache: HashMap<u32, Pvrz> = HashMap::new();
        for block in &v2.data_blocks {
            let pvrz = mos_v2_get_or_load_pvrz(&mut pvrz_cache, block, game_data)?;
            mos_v2_paste_block(block, pvrz, v2.width, image.as_mut());
        }
        Ok(Self {
            image,
            source: ImageSource::Mos { variant: v2.r#type },
        })
    }

    /// Image width in pixels. Mirrors `self.image.width()`.
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Image height in pixels. Mirrors `self.image.height()`.
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Short uppercase label of the source format (e.g. `"BMP"`,
    /// `"PVR"`). Useful for info bars where the format name is one
    /// column among several.
    pub fn format_label(&self) -> &'static str {
        match &self.source {
            ImageSource::Bmp { .. } => "BMP",
            ImageSource::Pvr { .. } => "PVR",
            ImageSource::Mos { .. } => "MOS",
        }
    }

    /// Human-readable description of the source format's relevant
    /// details — bit depth + compression for BMP, the DXT pixel-format
    /// variant for PVRZ. Suitable for direct display in a UI cell.
    pub fn format_description(&self) -> String {
        match &self.source {
            ImageSource::Bmp {
                bit_count,
                compression,
            } => format!("{bit_count} bpp, {compression}"),
            ImageSource::Pvr { pixel_format } => format!("{pixel_format:?}"),
            ImageSource::Mos { variant } => match variant {
                MosType::MosV1 => "V1".to_string(),
                MosType::Mosc => "MOSC".to_string(),
                MosType::MosV2 => "V2".to_string(),
            },
        }
    }
}

fn mos_v2_get_or_load_pvrz<'a>(
    cache: &'a mut HashMap<u32, Pvrz>,
    block: &MosV2DataBlock,
    game_data: &GameData,
) -> io::Result<&'a Pvrz> {
    if let std::collections::hash_map::Entry::Vacant(e) = cache.entry(block.pvrz_page) {
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
        let pvrz = PvrzImporter { name: &name }.import(ds)?;
        e.insert(pvrz);
    }
    Ok(cache.get(&block.pvrz_page).expect("just inserted"))
}

fn mos_v2_paste_block(
    block: &MosV2DataBlock,
    source_pvrz: &Pvrz,
    target_width: u32,
    target_buffer: &mut [u8],
) {
    let mut w = block.width;
    let mut h = block.height;
    if block.source_x_coordinate + w > source_pvrz.header.width {
        log::warn!(
            "PVRZ {} texture width out of bounds: src_x={} w={} texture_w={}",
            block.pvrz_name(),
            block.source_x_coordinate,
            w,
            source_pvrz.header.width
        );
        w = source_pvrz
            .header
            .width
            .saturating_sub(block.source_x_coordinate);
    }
    if block.source_y_coordinate + h > source_pvrz.header.height {
        log::warn!(
            "PVRZ {} texture height out of bounds: src_y={} h={} texture_h={}",
            block.pvrz_name(),
            block.source_y_coordinate,
            h,
            source_pvrz.header.height
        );
        h = source_pvrz
            .header
            .height
            .saturating_sub(block.source_y_coordinate);
    }
    if w == 0 || h == 0 {
        return;
    }

    let source_raw = source_pvrz.image.as_raw();
    for row in 0..h {
        let block_source_row = block.source_y_coordinate + row;
        let block_destination_row = block.target_y_coordinate + row;

        let source_start = ((block_source_row * source_pvrz.header.width
            + block.source_x_coordinate)
            * 4) as usize;
        let source_end = source_start + (w * 4) as usize;

        let target_start =
            ((block_destination_row * target_width + block.target_x_coordinate) * 4) as usize;
        let target_end = target_start + (w * 4) as usize;

        target_buffer[target_start..target_end]
            .copy_from_slice(&source_raw[source_start..source_end]);
    }
}

#[cfg(test)]
mod tests {
    use infinitier_bmp_resource::BmpImporter;
    use infinitier_common::Game;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_mos_resource::MosImporter;
    use infinitier_pvrz_resource::PvrzImporter;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    use super::*;

    #[test]
    fn test_from_bmp() {
        let ds = DataSource::new(get_assets_path().join("BMP/CCHAN05.BMP"));
        let bmp = BmpImporter { name: "test_bmp" }.import(&ds).unwrap();
        // Snapshot the BMP fields before `from_bmp` consumes the value.
        let original_w = bmp.image.width();
        let original_h = bmp.image.height();
        let original_bit_count = bmp.bit_count;
        let original_compression = bmp.compression;

        let img = ImportedImage::from_bmp(bmp);

        assert_eq!(img.width(), original_w);
        assert_eq!(img.height(), original_h);
        assert_eq!(img.format_label(), "BMP");
        match img.source {
            ImageSource::Bmp {
                bit_count,
                compression,
            } => {
                assert_eq!(bit_count, original_bit_count);
                assert_eq!(compression, original_compression);
            }
            other => panic!("expected ImageSource::Bmp, got {other:?}"),
        }
    }

    #[test]
    fn test_from_pvrz_dxt1() {
        let ds = DataSource::new(get_assets_path().join("PVR_DXT1/A004602.PVRZ"));
        let pvrz = PvrzImporter { name: "test_pvr" }.import(&ds).unwrap();
        // Capture header values before `from_pvrz` takes ownership.
        let (w, h) = (pvrz.header.width, pvrz.header.height);
        let pixel_format = pvrz.header.pixel_format;

        let img = ImportedImage::from_pvrz(pvrz);

        assert_eq!(img.width(), w);
        assert_eq!(img.height(), h);
        assert_eq!(img.format_label(), "PVR");
        assert!(matches!(img.source, ImageSource::Pvr { .. }));
        if let ImageSource::Pvr {
            pixel_format: actual,
        } = img.source
        {
            assert_eq!(actual, pixel_format);
        }
    }

    #[test]
    fn test_from_pvrz_dxt5() {
        let ds = DataSource::new(get_assets_path().join("PVR_DXT5/MOS0000.PVRZ"));
        let pvrz = PvrzImporter { name: "test_pvr" }.import(&ds).unwrap();
        let img = ImportedImage::from_pvrz(pvrz);

        assert_eq!(img.format_label(), "PVR");
        // Sanity: PVR_DXT5 fixture has DXT5 pixel format.
        match img.source {
            ImageSource::Pvr {
                pixel_format: PvrDataCompression::DXT5,
            } => {}
            other => panic!("expected DXT5 pixel format, got {other:?}"),
        }
    }

    #[test]
    fn test_format_description_bmp() {
        let bmp = Bmp {
            image: ImageBuffer::new(1, 1),
            bit_count: 24,
            compression: BmpCompression::Rgb,
        };
        let img = ImportedImage::from_bmp(bmp);
        assert_eq!(img.format_description(), "24 bpp, BI_RGB");
    }

    #[test]
    fn test_format_description_pvr() {
        // Build a PVR-flavoured ImportedImage manually so we don't have
        // to round-trip through DataSource just to test the description.
        let img = ImportedImage {
            image: ImageBuffer::new(1, 1),
            source: ImageSource::Pvr {
                pixel_format: PvrDataCompression::DXT5,
            },
        };
        assert_eq!(img.format_description(), "DXT5");
    }

    #[test]
    fn test_from_mos_v1() {
        // V1 needs no PVRZ lookup, so an empty GameData is fine.
        let game_data = GameData::new(vec![], Game::Bg2);
        let ds = DataSource::new(get_assets_path().join("MOS/V1/GTRSPCAP.mos"));
        let mos = MosImporter { name: "test_mos" }.import(&ds).unwrap();

        let img = ImportedImage::from_mos(mos, &game_data).unwrap();

        assert_eq!(img.width(), 5);
        assert_eq!(img.height(), 5);
        assert_eq!(img.format_label(), "MOS");
        assert!(matches!(
            img.source,
            ImageSource::Mos {
                variant: MosType::MosV1
            }
        ));
        assert_eq!(img.format_description(), "V1");
        assert_images_are_equal(
            &image::open(get_assets_path().join("MOS/V1/GTRSPCAP.png")).unwrap(),
            &img.image.into(),
            None,
        );
    }

    #[test]
    fn test_from_mos_mosc() {
        let game_data = GameData::new(vec![], Game::Bg2);
        let ds = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let mos = MosImporter { name: "test_mos" }.import(&ds).unwrap();

        let img = ImportedImage::from_mos(mos, &game_data).unwrap();

        assert_eq!(img.width(), 6);
        assert_eq!(img.height(), 600);
        assert_eq!(img.format_label(), "MOS");
        assert!(matches!(
            img.source,
            ImageSource::Mos {
                variant: MosType::Mosc
            }
        ));
        assert_eq!(img.format_description(), "MOSC");
        assert_images_are_equal(
            &image::open(get_assets_path().join("MOS/MOSC/GUIWRLP8.png")).unwrap(),
            &img.image.into(),
            None,
        );
    }

    #[test]
    fn test_from_mos_v2_missing_pvrz_errors() {
        // No PVRZ resources registered → V2's required `mosNNNN` lookup
        // fails. Documents the error surface for explorer / loader code.
        let game_data = GameData::new(vec![], Game::Bg2);
        let ds = DataSource::new(get_assets_path().join("MOS/V2/BGDECBAR.mos"));
        let mos = MosImporter { name: "test_mos" }.import(&ds).unwrap();
        let err = ImportedImage::from_mos(mos, &game_data).unwrap_err();
        assert!(
            err.to_string().to_ascii_uppercase().contains("MOS"),
            "expected error about missing PVRZ, got {err}"
        );
    }
}
