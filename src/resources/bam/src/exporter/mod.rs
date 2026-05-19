use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::{Bam, Type};

mod bamc;
mod v1;
mod v2;

/// A BAM file exporter.
///
/// Writes a [`Bam`] back to one of the three Infinity Engine BAM formats.
/// The output is functionally — not byte-exactly — equivalent to a source
/// archive: section offsets, the per-frame data layout, and the BAM V1
/// RLE-compression flag are chosen by the exporter rather than preserved.
/// Re-importing the emitted bytes yields a [`Bam`] whose `r#type`, frames,
/// cycles, palette, and (for V2) data blocks match the original.
///
/// Output format selection:
/// - [`Bam::V1`] with `r#type` = [`Type::BamV1`] → BAM V1 (uncompressed)
/// - [`Bam::V1`] with `r#type` = [`Type::BamC`] → BAMC (zlib-wrapped BAM V1)
/// - [`Bam::V2`] → BAM V2
pub struct BamExporter;

impl BamExporter {
    /// Writes `bam` to `writer`, picking the file format from `bam`'s tag.
    pub fn export<W: Write>(&self, bam: &Bam, writer: &mut W) -> io::Result<()> {
        match bam {
            Bam::V1(bam_v1) => match bam_v1.r#type {
                Type::BamV1 => {
                    let bytes = v1::build_bam_v1(bam_v1);
                    writer.write_all(&bytes)
                }
                Type::BamC => {
                    let bytes = v1::build_bam_v1(bam_v1);
                    bamc::write_bamc(&bytes, writer)
                }
                Type::BamV2 => Err(io::Error::other(
                    "Bam::V1 tagged as BamV2: invalid combination",
                )),
            },
            Bam::V2(bam_v2) => {
                let bytes = v2::build_bam_v2(bam_v2);
                writer.write_all(&bytes)
            }
        }
    }

    /// Writes `bam` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, bam: &Bam, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(bam, &mut writer)?;
        writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::BamImporter;
    use crate::{BamV1, BamV2};
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    fn roundtrip(path: &std::path::Path) -> (Bam, Bam) {
        let source = DataSource::new(path);
        let original = BamImporter { name: "bam_rt" }.import(&source).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        BamExporter.export(&original, &mut buf).unwrap();
        let re_imported = BamImporter { name: "bam_rt2" }
            .import(&DataSource::new(buf))
            .unwrap();
        (original, re_imported)
    }

    fn assert_bam_v1_content_equal(a: &BamV1, b: &BamV1) {
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.frames, b.frames, "frames");
        assert_eq!(a.cycles, b.cycles, "cycles");
        assert_eq!(a.palette, b.palette, "palette");
        assert_eq!(
            a.rle_compressed_color_index, b.rle_compressed_color_index,
            "rle_compressed_color_index"
        );
    }

    fn assert_bam_v2_content_equal(a: &BamV2, b: &BamV2) {
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.frames, b.frames, "frames");
        assert_eq!(a.cycles, b.cycles, "cycles");
        assert_eq!(a.data_blocks, b.data_blocks, "data_blocks");
    }

    #[test]
    fn test_roundtrip_bam_v1_single_frame() {
        let path = get_assets_path().join("BAM_V1/01/1chan03B_decompressed.BAM");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Bam::V1(a), Bam::V1(b)) => assert_bam_v1_content_equal(a, b),
            _ => panic!("expected Bam::V1"),
        }
    }

    #[test]
    fn test_roundtrip_bam_v1_multi_frame() {
        let path = get_assets_path().join("BAM_V1/02/SPHEART_decompressed.BAM");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Bam::V1(a), Bam::V1(b)) => assert_bam_v1_content_equal(a, b),
            _ => panic!("expected Bam::V1"),
        }
    }

    #[test]
    fn test_roundtrip_bam_v1_small() {
        let path = get_assets_path().join("BAM_V1/03/SPWI524D_decompressed.BAM");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Bam::V1(a), Bam::V1(b)) => assert_bam_v1_content_equal(a, b),
            _ => panic!("expected Bam::V1"),
        }
    }

    #[test]
    fn test_roundtrip_bamc() {
        // The BAMC-tagged BamV1 should round-trip back to BAMC.
        let path = get_assets_path().join("BAM_V1/02/SPHEART_compressed.BAM");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Bam::V1(a), Bam::V1(b)) => {
                assert_eq!(a.r#type, Type::BamC);
                assert_eq!(b.r#type, Type::BamC);
                assert_bam_v1_content_equal(a, b);
            }
            _ => panic!("expected Bam::V1"),
        }
    }

    #[test]
    fn test_roundtrip_bam_v2() {
        let path = get_assets_path().join("BAM_V2/1CHELM03.BAM");
        let (original, re_imported) = roundtrip(&path);
        match (&original, &re_imported) {
            (Bam::V2(a), Bam::V2(b)) => assert_bam_v2_content_equal(a, b),
            _ => panic!("expected Bam::V2"),
        }
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let path = get_assets_path().join("BAM_V1/03/SPWI524D_decompressed.BAM");
        let source = DataSource::new(path.as_path());
        let original = BamImporter {
            name: "bam_file_rt",
        }
        .import(&source)
        .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        BamExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = BamImporter {
            name: "bam_file_rt2",
        }
        .import(&DataSource::new(tmp.path().to_path_buf()))
        .unwrap();
        match (&original, &re_imported) {
            (Bam::V1(a), Bam::V1(b)) => assert_bam_v1_content_equal(a, b),
            _ => panic!("expected Bam::V1"),
        }
    }
}
