use image::{ImageBuffer, Rgba};
use log::debug;

use infinitier_datasource::{DataSource, Importer};

/// A BMP file importer
pub struct BmpImporter<'a> {
    pub name: &'a str,
}

impl <'a> Importer for BmpImporter<'a> {
    type T = Bmp;

    fn import(&self, source: &DataSource) -> std::io::Result<Bmp> {
        let mut reader = source.reader()?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        decode_bmp(&bytes, &self.name)
    }
}

fn decode_bmp(bytes: &[u8], name: &str) -> std::io::Result<Bmp> {
    let (bit_count, compression) = parse_bmp_header(bytes);

    if let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp) {
        let image = img.to_rgba8();
        debug!("Loaded {name} [BMP]: {}x{}", image.width(), image.height());
        return Ok(Bmp {
            image,
            bit_count,
            compression,
        });
    }

    // Some BMP files (e.g. original BG) declare more palette entries than the
    // bit depth allows. Patch clr_used at header offset 46 to cap it at 2^bit_count.
    if bytes.len() >= 50 && bit_count > 0 && bit_count <= 8 {
        let max_colors = 1u32 << bit_count;
        let clr_used = u32::from_le_bytes(bytes[46..50].try_into().unwrap());
        if clr_used > max_colors {
            let mut patched = bytes.to_vec();
            patched[46..50].copy_from_slice(&max_colors.to_le_bytes());
            let image = image::load_from_memory_with_format(&patched, image::ImageFormat::Bmp)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                .to_rgba8();
            debug!(
                "Loaded {name} [BMP] (patched palette {} -> {}): {}x{}",
                clr_used,
                max_colors,
                image.width(),
                image.height()
            );
            return Ok(Bmp {
                image,
                bit_count,
                compression,
            });
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Failed to decode BMP",
    ))
}

/// Returns `(bit_count, compression)` from a BITMAPINFOHEADER-style DIB header,
/// or sentinel values when the header is too short to read.
fn parse_bmp_header(bytes: &[u8]) -> (u16, BmpCompression) {
    if bytes.len() < 34 {
        return (0, BmpCompression::Other(0));
    }
    let bit_count = u16::from_le_bytes([bytes[28], bytes[29]]);
    let compression =
        BmpCompression::from_u32(u32::from_le_bytes(bytes[30..34].try_into().unwrap()));
    (bit_count, compression)
}

/// BMP DIB-header compression scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmpCompression {
    Rgb,
    Rle8,
    Rle4,
    Bitfields,
    Jpeg,
    Png,
    Other(u32),
}

impl BmpCompression {
    fn from_u32(v: u32) -> Self {
        match v {
            0 => Self::Rgb,
            1 => Self::Rle8,
            2 => Self::Rle4,
            3 => Self::Bitfields,
            4 => Self::Jpeg,
            5 => Self::Png,
            n => Self::Other(n),
        }
    }
}

impl std::fmt::Display for BmpCompression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rgb => f.write_str("BI_RGB"),
            Self::Rle8 => f.write_str("BI_RLE8"),
            Self::Rle4 => f.write_str("BI_RLE4"),
            Self::Bitfields => f.write_str("BI_BITFIELDS"),
            Self::Jpeg => f.write_str("BI_JPEG"),
            Self::Png => f.write_str("BI_PNG"),
            Self::Other(n) => write!(f, "Unknown({n})"),
        }
    }
}

/// A BMP file
#[derive(Debug)]
pub struct Bmp {
    /// Decoded pixels, always upgraded to RGBA8 regardless of the source format.
    pub image: ImageBuffer<Rgba<u8>, Vec<u8>>,
    /// Original bit depth as declared in the DIB header (1, 4, 8, 16, 24, 32).
    pub bit_count: u16,
    /// Compression scheme as declared in the DIB header.
    pub compression: BmpCompression,
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    #[test]
    fn test_parse_bmp_01() {
        let data = DataSource::new(get_assets_path().join("resources/BMP/CCHAN05.BMP"));

        let original = image::open(get_assets_path().join("resources/BMP/CCHAN05.BMP")).unwrap();

        let bmp = BmpImporter{name: "bmp_name"}.import(&data).unwrap();

        assert_images_are_equal(&bmp.image.clone().into(), &original);
    }

    #[test]
    fn test_parse_bmp_02() {
        let data = DataSource::new(get_assets_path().join("resources/BMP/MINSCM.BMP"));

        let original = image::open(get_assets_path().join("resources/BMP/MINSCM.BMP")).unwrap();

        let bmp = BmpImporter{name: "bmp_name"}.import(&data).unwrap();

        assert_images_are_equal(&bmp.image.clone().into(), &original);
    }

    #[test]
    fn metadata_for_32bpp_bmp() {
        let data = DataSource::new(get_assets_path().join("resources/BMP/CCHAN05.BMP"));
        let bmp = BmpImporter{name: "bmp_name"}.import(&data).unwrap();
        assert_eq!(bmp.bit_count, 32);
    }

    #[test]
    fn metadata_for_24bpp_bmp() {
        let data = DataSource::new(get_assets_path().join("resources/BMP/MINSCM.BMP"));
        let bmp = BmpImporter{name: "bmp_name"}.import(&data).unwrap();
        assert_eq!(bmp.bit_count, 24);
        assert_eq!(bmp.compression, BmpCompression::Rgb);
    }
}
