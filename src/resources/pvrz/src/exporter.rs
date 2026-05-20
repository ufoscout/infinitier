use std::io::Write;

use flate2::write::ZlibEncoder;
use image::{ImageBuffer, Rgba};

use crate::{PvrDataCompression, Pvrz};

/// A PVRZ file exporter.
///
/// Writes an `ImageBuffer` back as a PVRZ (zlib-compressed PVR) byte stream.
/// Note that the PVRZ format is lossy.
pub struct PvrzExporter {
    pub format: PvrDataCompression,
}

impl PvrzExporter {
    /// Exports an image to a PVRZ byte stream.
    pub fn export<W: Write>(
        &self,
        image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        writer: &mut W,
    ) -> std::io::Result<()> {
        let width = image.width();
        let height = image.height();

        // Pre-zlib u32: purpose unclear (gemrb uses it as an endianness marker).
        // The importer discards it; emit 0.
        writer.write_all(&0u32.to_le_bytes())?;

        let tex_format = match self.format {
            PvrDataCompression::DXT1 => texpresso::Format::Bc1,
            PvrDataCompression::DXT5 => texpresso::Format::Bc3,
        };
        let mut compressed = vec![0u8; tex_format.compressed_size(width as usize, height as usize)];
        tex_format.compress(
            image.as_raw(),
            width as usize,
            height as usize,
            texpresso::Params::default(),
            &mut compressed,
        );

        let mut zlib = ZlibEncoder::new(writer, flate2::Compression::default());

        // 52-byte PVR header (little-endian).
        zlib.write_all(&0x03525650u32.to_le_bytes())?; // version ("PVR\x03")
        zlib.write_all(&0u32.to_le_bytes())?; // flags
        zlib.write_all(&self.format.to_u64().to_le_bytes())?; // pixel_format (u64)
        zlib.write_all(&0u32.to_le_bytes())?; // color_space
        zlib.write_all(&0u32.to_le_bytes())?; // channel_type
        zlib.write_all(&height.to_le_bytes())?;
        zlib.write_all(&width.to_le_bytes())?;
        zlib.write_all(&1u32.to_le_bytes())?; // depth
        zlib.write_all(&1u32.to_le_bytes())?; // surfaces_number
        zlib.write_all(&1u32.to_le_bytes())?; // faces_number
        zlib.write_all(&1u32.to_le_bytes())?; // mip_map_count
        zlib.write_all(&0u32.to_le_bytes())?; // metadata_size

        zlib.write_all(&compressed)?;
        zlib.finish()?;
        Ok(())
    }

    /// Exports an image to a PVRZ file at `filename`.
    pub fn export_to_file<P: AsRef<std::path::Path>>(
        &self,
        image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        filename: P,
    ) -> std::io::Result<()> {
        let file = std::fs::File::create(filename)?;
        let mut writer = std::io::BufWriter::new(file);
        self.export(image, &mut writer)?;
        writer.flush()?;
        Ok(())
    }

    /// Exports a previously-imported [`Pvrz`] to a PVRZ byte stream.
    ///
    /// The compression format is taken from `pvrz.header.pixel_format` so
    /// the round-trip preserves the original DXT1/DXT5 choice. Most other
    /// header fields are not preserved verbatim — see [`PvrzExporter::export`]
    /// for the fixed values written (version, mip_map_count = 1, …).
    pub fn export_pvrz<W: Write>(pvrz: &Pvrz, writer: &mut W) -> std::io::Result<()> {
        PvrzExporter {
            format: pvrz.header.pixel_format,
        }
        .export(&pvrz.image, writer)
    }

    /// Exports a previously-imported [`Pvrz`] to a PVRZ file at `filename`.
    pub fn export_pvrz_to_file<P: AsRef<std::path::Path>>(
        pvrz: &Pvrz,
        filename: P,
    ) -> std::io::Result<()> {
        PvrzExporter {
            format: pvrz.header.pixel_format,
        }
        .export_to_file(&pvrz.image, filename)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{PvrzHeader, PvrzImporter};
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    #[test]
    fn test_export_dxt1_roundtrip() {
        let data = DataSource::new(get_assets_path().join("PVR_DXT1/A004602.PVRZ"));
        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        let mut buf: Vec<u8> = Vec::new();
        PvrzExporter {
            format: pvrz.header.pixel_format,
        }
        .export(&pvrz.image, &mut buf)
        .unwrap();

        let pvrz2 = PvrzImporter { name: "rt" }
            .import(&DataSource::new(buf))
            .unwrap();
        assert_eq!(
            pvrz2.header,
            PvrzHeader {
                version: 0x03525650,
                flags: 0,
                pixel_format: PvrDataCompression::DXT1,
                color_space: 0,
                channel_type: 0,
                height: 1024,
                width: 256,
                depth: 1,
                surfaces_number: 1,
                faces_number: 1,
                mip_map_count: 1,
                metadata_size: 0,
            }
        );

        // BC1 is lossy but losses on this asset stay within a few levels.
        assert_images_are_equal(&pvrz.image.into(), &pvrz2.image.into(), Some(8));
    }

    #[test]
    fn test_export_dxt5_roundtrip() {
        let data = DataSource::new(get_assets_path().join("PVR_DXT5/MOS0000.PVRZ"));
        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        let mut buf: Vec<u8> = Vec::new();
        PvrzExporter {
            format: pvrz.header.pixel_format,
        }
        .export(&pvrz.image, &mut buf)
        .unwrap();

        let pvrz2 = PvrzImporter { name: "rt" }
            .import(&DataSource::new(buf))
            .unwrap();
        assert_eq!(
            pvrz2.header,
            PvrzHeader {
                version: 0x03525650,
                flags: 0,
                pixel_format: PvrDataCompression::DXT5,
                color_space: 0,
                channel_type: 0,
                height: 512,
                width: 512,
                depth: 1,
                surfaces_number: 1,
                faces_number: 1,
                mip_map_count: 1,
                metadata_size: 0,
            }
        );

        // BC3's 8-bit alpha block quantizes far more coarsely than the
        // 565 color block, so alpha needs a much wider tolerance.
        assert_images_are_equal(&pvrz.image.into(), &pvrz2.image.into(), Some(40));
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let data = DataSource::new(get_assets_path().join("PVR_DXT1/A004602.PVRZ"));
        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        PvrzExporter {
            format: pvrz.header.pixel_format,
        }
        .export_to_file(&pvrz.image, tmp.path())
        .unwrap();

        let pvrz2 = PvrzImporter { name: "rt" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        assert_eq!(pvrz2.header.width, pvrz.image.width());
        assert_eq!(pvrz2.header.height, pvrz.image.height());
        assert_eq!(pvrz2.header.pixel_format, PvrDataCompression::DXT1);

        assert_images_are_equal(&pvrz.image.into(), &pvrz2.image.into(), Some(8));
    }

    #[test]
    fn test_export_pvrz_roundtrip() {
        // DXT5 source: export_pvrz should infer the format from the
        // header and produce a byte stream that re-imports to the same
        // PVRZ (compression-format preserved, image within DXT5 lossy
        // tolerance).
        let data = DataSource::new(get_assets_path().join("PVR_DXT5/MOS0000.PVRZ"));
        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        let mut buf: Vec<u8> = Vec::new();
        PvrzExporter::export_pvrz(&pvrz, &mut buf).unwrap();

        let pvrz2 = PvrzImporter { name: "rt" }
            .import(&DataSource::new(buf))
            .unwrap();
        assert_eq!(pvrz2.header.pixel_format, PvrDataCompression::DXT5);
        assert_eq!(pvrz2.header.width, pvrz.image.width());
        assert_eq!(pvrz2.header.height, pvrz.image.height());
        assert_images_are_equal(&pvrz.image.into(), &pvrz2.image.into(), Some(40));
    }

    #[test]
    fn test_export_pvrz_to_file_roundtrip() {
        let data = DataSource::new(get_assets_path().join("PVR_DXT1/A004602.PVRZ"));
        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        PvrzExporter::export_pvrz_to_file(&pvrz, tmp.path()).unwrap();

        let pvrz2 = PvrzImporter { name: "rt" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        assert_eq!(pvrz2.header.pixel_format, PvrDataCompression::DXT1);
        assert_eq!(pvrz2.header.width, pvrz.image.width());
        assert_eq!(pvrz2.header.height, pvrz.image.height());
        assert_images_are_equal(&pvrz.image.into(), &pvrz2.image.into(), Some(8));
    }
}
