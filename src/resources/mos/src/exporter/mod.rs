use std::io::{self, BufWriter, Write};
use std::path::Path;

use image::{ImageBuffer, Rgba};

use crate::{Mos, Type};

mod mosc;
mod v1;
mod v2;

pub use v1::image_to_mos_v1;

/// A MOS file exporter.
///
/// Writes a [`Mos`] back to one of the three Infinity Engine MOS formats.
/// Like [`infinitier_bam_resource::BamExporter`], the output is
/// functionally — not byte-exactly — equivalent to a source archive:
/// section offsets and per-block data layout are chosen by the exporter
/// rather than preserved. Re-importing the emitted bytes yields a [`Mos`]
/// whose `r#type`, dimensions, blocks, and palettes match the original.
///
/// Output format selection:
/// - [`Mos::V1`] with `r#type` = [`Type::MosV1`] → MOS V1 (uncompressed)
/// - [`Mos::V1`] with `r#type` = [`Type::Mosc`] → MOSC (zlib-wrapped MOS V1)
/// - [`Mos::V2`] → MOS V2
///
/// Use [`MosExporter::image_to_mos_v1`] to convert an arbitrary RGBA
/// image into a [`MosV1`](crate::MosV1) suitable for `export()`.
pub struct MosExporter;

impl MosExporter {
    /// Writes `mos` to `writer`, picking the file format from `mos`'s tag.
    pub fn export<W: Write>(&self, mos: &Mos, writer: &mut W) -> io::Result<()> {
        match mos {
            Mos::V1(mos_v1) => match mos_v1.r#type {
                Type::MosV1 => {
                    let bytes = v1::build_mos_v1(mos_v1);
                    writer.write_all(&bytes)
                }
                Type::Mosc => {
                    let bytes = v1::build_mos_v1(mos_v1);
                    mosc::write_mosc(&bytes, writer)
                }
                Type::MosV2 => Err(io::Error::other(
                    "Mos::V1 tagged as MosV2: invalid combination",
                )),
            },
            Mos::V2(mos_v2) => {
                let bytes = v2::build_mos_v2(mos_v2);
                writer.write_all(&bytes)
            }
        }
    }

    /// Writes `mos` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, mos: &Mos, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(mos, &mut writer)?;
        writer.flush()
    }

    /// Converts an arbitrary RGBA image into a [`MosV1`](crate::MosV1)
    /// struct ready for [`MosExporter::export`].
    ///
    /// Splits the image into 64×64 blocks (the last column/row may be
    /// truncated to fit the overall dimensions) and quantizes each block
    /// independently to a 256-entry palette via NeuQuant. Pixels with
    /// `alpha == 0` are mapped to the MOS V1 "magic green" transparent
    /// palette entry — `RGB(0, 255, 0)` — preserving transparency through
    /// the round-trip with [`MosV1::to_image`](crate::MosV1::to_image).
    ///
    /// To emit MOSC instead of plain MOS V1, set the returned value's
    /// `r#type` to [`Type::Mosc`] before passing it back through `export`.
    pub fn image_to_mos_v1(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> crate::MosV1 {
        image_to_mos_v1(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::MosImporter;
    use crate::{Mos, MosV1, MosV2, Type};
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    fn roundtrip(path: &std::path::Path) -> (Mos, Mos) {
        let source = DataSource::new(path);
        let original = MosImporter { name: "mos_rt" }.import(&source).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        MosExporter.export(&original, &mut buf).unwrap();
        let re_imported = MosImporter { name: "mos_rt2" }
            .import(&DataSource::new(buf))
            .unwrap();
        (original, re_imported)
    }

    fn assert_mos_v1_content_equal(a: &MosV1, b: &MosV1) {
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.width, b.width, "width");
        assert_eq!(a.height, b.height, "height");
        assert_eq!(a.columns, b.columns, "columns");
        assert_eq!(a.rows, b.rows, "rows");
        assert_eq!(a.blocks, b.blocks, "blocks");
    }

    fn assert_mos_v2_content_equal(a: &MosV2, b: &MosV2) {
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.width, b.width, "width");
        assert_eq!(a.height, b.height, "height");
        assert_eq!(a.data_blocks, b.data_blocks, "data_blocks");
    }

    #[test]
    fn test_roundtrip_mos_v1_gtrspcap() {
        let path = get_assets_path().join("MOS/V1/GTRSPCAP.mos");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Mos::V1(a), Mos::V1(b)) => {
                assert_eq!(a.r#type, Type::MosV1);
                assert_mos_v1_content_equal(a, b);
            }
            _ => panic!("expected Mos::V1"),
        }
    }

    #[test]
    fn test_roundtrip_mosc_guiwrlp8() {
        // MOSC source must round-trip back to MOSC.
        let path = get_assets_path().join("MOS/MOSC/GUIWRLP8.mos");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Mos::V1(a), Mos::V1(b)) => {
                assert_eq!(a.r#type, Type::Mosc);
                assert_eq!(b.r#type, Type::Mosc);
                assert_mos_v1_content_equal(a, b);
            }
            _ => panic!("expected Mos::V1"),
        }
    }

    #[test]
    fn test_roundtrip_mos_v2_bgdecbar() {
        let path = get_assets_path().join("MOS/V2/BGDECBAR.mos");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Mos::V2(a), Mos::V2(b)) => {
                assert_eq!(a.r#type, Type::MosV2);
                assert_mos_v2_content_equal(a, b);
            }
            _ => panic!("expected Mos::V2"),
        }
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let path = get_assets_path().join("MOS/V1/GTRSPCAP.mos");
        let source = DataSource::new(path.as_path());
        let original = MosImporter {
            name: "mos_file_rt",
        }
        .import(&source)
        .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        MosExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = MosImporter {
            name: "mos_file_rt2",
        }
        .import(&DataSource::new(tmp.path().to_path_buf()))
        .unwrap();
        match (&original, &re_imported) {
            (Mos::V1(a), Mos::V1(b)) => assert_mos_v1_content_equal(a, b),
            _ => panic!("expected Mos::V1"),
        }
    }

    #[test]
    fn test_image_to_mos_v1_matches_source_image() {
        // image → MOS V1 → image must reproduce the original image.
        // GTRSPCAP is 5×5 (≤256 unique colors) so no quantization loss.
        let png = image::open(get_assets_path().join("MOS/V1/GTRSPCAP.png"))
            .unwrap()
            .to_rgba8();
        let mos_v1 = MosExporter::image_to_mos_v1(&png);
        assert_eq!(mos_v1.width, 5);
        assert_eq!(mos_v1.height, 5);
        assert_eq!(mos_v1.columns, 1);
        assert_eq!(mos_v1.rows, 1);
        assert_eq!(mos_v1.blocks.len(), 1);
        assert_images_are_equal(&png.into(), &mos_v1.to_image().into(), None);
    }

    #[test]
    fn test_image_to_mos_v1_roundtrip_via_file() {
        // Full image → MosV1 → bytes → MosV1 → image round trip.
        let png = image::open(get_assets_path().join("MOS/V1/GTRSPCAP.png"))
            .unwrap()
            .to_rgba8();
        let mos_v1 = MosExporter::image_to_mos_v1(&png);
        let mos = Mos::V1(mos_v1);

        let mut buf: Vec<u8> = Vec::new();
        MosExporter.export(&mos, &mut buf).unwrap();
        let re_imported = MosImporter { name: "img_rt" }
            .import(&DataSource::new(buf))
            .unwrap();
        let Mos::V1(re) = re_imported else {
            panic!("expected Mos::V1");
        };
        assert_images_are_equal(&png.into(), &re.to_image().into(), None);
    }

    #[test]
    fn test_image_to_mos_v1_multi_block() {
        // MOSC source is 6×600 → 1 column × 10 rows. Decode it to an
        // image, run that through image_to_mos_v1, and verify the result
        // round-trips back to the same image — this exercises the block
        // grid, last-row truncation, and per-block palette generation.
        let data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        let original = MosImporter { name: "src" }.import(&data).unwrap();
        let Mos::V1(original_v1) = original else {
            panic!("expected Mos::V1");
        };
        let img = original_v1.to_image();

        let rebuilt = MosExporter::image_to_mos_v1(&img);
        assert_eq!(rebuilt.width, 6);
        assert_eq!(rebuilt.height, 600);
        assert_eq!(rebuilt.columns, 1);
        assert_eq!(rebuilt.rows, 10);
        assert_eq!(rebuilt.blocks.len(), 10);
        assert_images_are_equal(&img.into(), &rebuilt.to_image().into(), None);
    }

    #[test]
    fn test_image_to_mos_v1_as_mosc() {
        // Promote the rebuilt MosV1 to MOSC and confirm the wrapper
        // round-trips through export()/import().
        let png = image::open(get_assets_path().join("MOS/V1/GTRSPCAP.png"))
            .unwrap()
            .to_rgba8();
        let mut mos_v1 = MosExporter::image_to_mos_v1(&png);
        mos_v1.r#type = Type::Mosc;

        let mut buf: Vec<u8> = Vec::new();
        MosExporter.export(&Mos::V1(mos_v1), &mut buf).unwrap();
        let re_imported = MosImporter { name: "mosc_rt" }
            .import(&DataSource::new(buf))
            .unwrap();
        let Mos::V1(re) = re_imported else {
            panic!("expected Mos::V1");
        };
        assert_eq!(re.r#type, Type::Mosc);
        assert_images_are_equal(&png.into(), &re.to_image().into(), None);
    }
}
