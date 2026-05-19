//! Unified image abstraction over BMP / PVRZ (and, later, TGA / PNG /
//! MOS / TIS …). Viewers and game code that just want "RGBA8 pixels
//! plus a label" should work against [`ImportedImage`] instead of each
//! format's bespoke importer output, so adding a new image format is a
//! single new constructor here rather than a fresh viewer.

use std::io;

use image::{ImageBuffer, Rgba};
use infinitier_bmp_resource::{Bmp, BmpCompression};
use infinitier_datasource::DataSource;
use infinitier_pvrz_resource::{PvrDataCompression, PvrzHeader, PvrzImporter};

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

    /// Decode a PVRZ from its parsed header plus the data source the
    /// header was read from. The pixel data is decompressed eagerly
    /// (DXT1 or DXT5 → RGBA8) so the returned image is ready to upload
    /// to the GPU.
    pub fn from_pvrz(header: PvrzHeader, source: &DataSource) -> io::Result<Self> {
        let image = PvrzImporter::to_image(&header, source).map_err(io::Error::other)?;
        Ok(Self {
            image,
            source: ImageSource::Pvr {
                pixel_format: header.pixel_format,
            },
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
        }
    }
}

#[cfg(test)]
mod tests {
    use infinitier_bmp_resource::BmpImporter;
    use infinitier_datasource::Importer;
    use infinitier_test_utils::get_assets_path;

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
        let header = PvrzImporter { name: "test_pvr" }.import(&ds).unwrap();
        // Capture header values before `from_pvrz` takes ownership.
        let (w, h) = (header.width, header.height);
        let pixel_format = header.pixel_format;

        let img = ImportedImage::from_pvrz(header, &ds).unwrap();

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
        let header = PvrzImporter { name: "test_pvr" }.import(&ds).unwrap();
        let img = ImportedImage::from_pvrz(header, &ds).unwrap();

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
}
