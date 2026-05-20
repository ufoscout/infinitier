use std::io::{self, BufWriter, Write};
use std::path::Path;

use image::{ImageBuffer, Rgba};

use crate::Tis;

mod palette;
mod pvrz;

pub use palette::image_to_tis_palette;

/// A TIS file exporter.
///
/// Writes a [`Tis`] back to its on-disk form. Like
/// [`infinitier_bam_resource::BamExporter`] /
/// [`infinitier_mos_resource::MosExporter`], the output is functionally —
/// not byte-exactly — equivalent to the source archive: header offsets
/// and per-tile data layout are chosen by the exporter rather than
/// preserved. Re-importing the emitted bytes yields a [`Tis`] whose
/// `tile_dimension` and tile contents match the original.
///
/// Output format is dispatched on the [`Tis`] variant — palette tiles
/// emit a paletted archive, PVRZ tiles emit a metadata-only archive
/// (both with the shared `"TIS V1  "` signature).
///
/// Use [`TisExporter::image_to_tis_palette`] to convert an arbitrary
/// RGBA image into a [`TisPalette`](crate::TisPalette) suitable for
/// `export()`.
pub struct TisExporter;

impl TisExporter {
    /// Writes `tis` to `writer`, picking the variant from the [`Tis`]
    /// outer enum.
    pub fn export<W: Write>(&self, tis: &Tis, writer: &mut W) -> io::Result<()> {
        match tis {
            Tis::Palette(p) => {
                let bytes = palette::build_tis_palette(p);
                writer.write_all(&bytes)
            }
            Tis::Pvrz(p) => {
                let bytes = pvrz::build_tis_pvrz(p);
                writer.write_all(&bytes)
            }
        }
    }

    /// Writes `tis` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, tis: &Tis, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(tis, &mut writer)?;
        writer.flush()
    }

    /// Converts an arbitrary RGBA image into a
    /// [`TisPalette`](crate::TisPalette) struct ready for
    /// [`TisExporter::export`].
    ///
    /// Splits the image into 64×64 tiles laid out row-major (left-to-right,
    /// top-to-bottom) and quantizes each tile independently to a 256-entry
    /// palette via NeuQuant. Pixels with `alpha == 0` are routed to a
    /// reserved transparency entry at palette index 0 holding the magic
    /// green `RGB(0, 255, 0)` — matching NearInfinity's TIS V1 rendering
    /// rule.
    ///
    /// Returns an error if the image's width or height isn't a positive
    /// multiple of 64: TIS tiles are fixed-size and the on-disk format
    /// has no notion of a partial tile.
    pub fn image_to_tis_palette(
        image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ) -> io::Result<crate::TisPalette> {
        image_to_tis_palette(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::TisImporter;
    use crate::{Tis, TisPalette, TisPvrz, Type};
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    fn roundtrip(path: &std::path::Path) -> (Tis, Tis) {
        let source = DataSource::new(path);
        let original = TisImporter { name: "tis_rt" }.import(&source).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        TisExporter.export(&original, &mut buf).unwrap();
        let re_imported = TisImporter { name: "tis_rt2" }
            .import(&DataSource::new(buf))
            .unwrap();
        (original, re_imported)
    }

    fn assert_palette_equal(a: &TisPalette, b: &TisPalette) {
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.tile_dimension, b.tile_dimension, "tile_dimension");
        assert_eq!(a.tiles, b.tiles, "tiles");
    }

    fn assert_pvrz_equal(a: &TisPvrz, b: &TisPvrz) {
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.tile_dimension, b.tile_dimension, "tile_dimension");
        assert_eq!(a.tiles, b.tiles, "tiles");
    }

    #[test]
    fn test_roundtrip_palette_fire01() {
        let (original, re_imported) = roundtrip(&get_assets_path().join("TIS/Palette/FIRE01.tis"));
        match (&original, &re_imported) {
            (Tis::Palette(a), Tis::Palette(b)) => {
                assert_eq!(a.r#type, Type::Palette);
                assert_palette_equal(a, b);
            }
            _ => panic!("expected Tis::Palette"),
        }
    }

    #[test]
    fn test_roundtrip_palette_hotcoalr() {
        let (original, re_imported) =
            roundtrip(&get_assets_path().join("TIS/Palette/HOTCOALR.tis"));
        match (&original, &re_imported) {
            (Tis::Palette(a), Tis::Palette(b)) => assert_palette_equal(a, b),
            _ => panic!("expected Tis::Palette"),
        }
    }

    #[test]
    fn test_roundtrip_pvrz_ar0107() {
        let (original, re_imported) = roundtrip(&get_assets_path().join("TIS/Pvrz/AR0107.tis"));
        match (&original, &re_imported) {
            (Tis::Pvrz(a), Tis::Pvrz(b)) => {
                assert_eq!(a.r#type, Type::Pvrz);
                assert_pvrz_equal(a, b);
            }
            _ => panic!("expected Tis::Pvrz"),
        }
    }

    #[test]
    fn test_roundtrip_palette_bytes_are_byte_exact() {
        // Palette TIS has no compression and no optional fields, so the
        // exporter's output should be byte-for-byte identical to the
        // source file (24-byte header, then tiles back-to-back).
        let path = get_assets_path().join("TIS/Palette/FIRE01.tis");
        let original_bytes = std::fs::read(&path).unwrap();
        let tis = TisImporter { name: "byte_rt" }
            .import(&DataSource::new(path))
            .unwrap();
        let mut produced: Vec<u8> = Vec::new();
        TisExporter.export(&tis, &mut produced).unwrap();
        assert_eq!(produced, original_bytes);
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let path = get_assets_path().join("TIS/Palette/HOTCOALR.tis");
        let original = TisImporter { name: "file_rt" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        TisExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = TisImporter { name: "file_rt2" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        match (&original, &re_imported) {
            (Tis::Palette(a), Tis::Palette(b)) => assert_palette_equal(a, b),
            _ => panic!("expected Tis::Palette"),
        }
    }

    #[test]
    fn test_image_to_tis_palette_matches_source_image() {
        // The HOTCOALR PNG is 384×64 — six 64×64 tiles in a single
        // row. Importing it and round-tripping via image_to_tis_palette
        // + to_image must reproduce the source image byte-exactly,
        // because each tile has ≤256 unique colors (direct palette).
        let png = image::open(get_assets_path().join("TIS/Palette/HOTCOALR.png"))
            .unwrap()
            .to_rgba8();
        let tis = TisExporter::image_to_tis_palette(&png).unwrap();
        assert_eq!(tis.r#type, Type::Palette);
        assert_eq!(tis.tile_dimension, 64);
        assert_eq!(tis.tiles.len(), 6);
        assert_images_are_equal(&png.into(), &tis.to_image(6).into(), None);
    }

    #[test]
    fn test_image_to_tis_palette_then_export() {
        // Full image → TisPalette → bytes → TisPalette → image: must
        // reproduce the source pixel-perfectly when colors fit in the
        // 256-entry palettes (HOTCOALR does).
        let png = image::open(get_assets_path().join("TIS/Palette/HOTCOALR.png"))
            .unwrap()
            .to_rgba8();
        let tis = TisExporter::image_to_tis_palette(&png).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        TisExporter.export(&Tis::Palette(tis), &mut buf).unwrap();
        let re_imported = TisImporter { name: "img_rt" }
            .import(&DataSource::new(buf))
            .unwrap();
        let Tis::Palette(re) = re_imported else {
            panic!("expected Tis::Palette");
        };
        assert_eq!(re.tiles.len(), 6);
        assert_images_are_equal(&png.into(), &re.to_image(6).into(), None);
    }

    #[test]
    fn test_image_to_tis_palette_rejects_non_multiple_of_64() {
        // 100×64 is not a multiple of 64 on the width axis — TIS can't
        // hold a partial tile, so the conversion fails.
        let img = image::ImageBuffer::<Rgba<u8>, _>::new(100, 64);
        let err = TisExporter::image_to_tis_palette(&img).unwrap_err();
        assert!(
            err.to_string().contains("multiples of 64"),
            "got error: {err}"
        );
    }

    #[test]
    fn test_image_to_tis_palette_rejects_empty() {
        let img = image::ImageBuffer::<Rgba<u8>, _>::new(0, 0);
        let err = TisExporter::image_to_tis_palette(&img).unwrap_err();
        assert!(err.to_string().contains("non-zero"), "got error: {err}");
    }

    #[test]
    fn test_image_to_tis_palette_preserves_transparency() {
        // Construct a 64×64 image where the top-left pixel is fully
        // transparent and the rest is opaque red. After round-tripping
        // through TIS, the transparency should be preserved via the
        // reserved palette[0] = magic-green slot.
        let mut img = image::ImageBuffer::<Rgba<u8>, _>::new(64, 64);
        for (_, _, px) in img.enumerate_pixels_mut() {
            *px = Rgba([255, 0, 0, 255]);
        }
        img.put_pixel(0, 0, Rgba([0, 0, 0, 0]));

        let tis = TisExporter::image_to_tis_palette(&img).unwrap();
        let rendered = tis.to_image(1);
        // Renderer keeps the magic-green RGB and zeros alpha — same
        // convention as `MosV1::to_image`. What matters for the engine
        // is `alpha == 0`; downstream consumers using premultiplied
        // alpha will see this as transparent regardless of RGB.
        assert_eq!(rendered.get_pixel(0, 0).0, [0, 255, 0, 0]);
        assert_eq!(rendered.get_pixel(63, 63).0, [255, 0, 0, 255]);

        // Slot 0 must hold the magic green so the renderer treats it as
        // transparent.
        let p0 = tis.tiles[0].palette[0];
        assert_eq!((p0.r, p0.g, p0.b), (0, 255, 0));
    }
}
