#![doc = include_str!("../readme.md")]

use std::io::Read;

use image::{ImageBuffer, Rgba};
use log::debug;

use infinitier_datasource::{DataSource, Importer};

/// 8-byte PNG signature (per ISO/IEC 15948 §5.2). A file starting with
/// these bytes is unambiguously a PNG.
pub const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// A PNG file importer.
pub struct PngImporter<'a> {
    pub name: &'a str,
}

impl<'a> Importer for PngImporter<'a> {
    type T = Png;

    fn import(&self, source: &DataSource) -> std::io::Result<Png> {
        let mut reader = source.reader()?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        decode_png(&bytes, self.name)
    }
}

fn decode_png(bytes: &[u8], name: &str) -> std::io::Result<Png> {
    let (bit_depth, color_type) = parse_ihdr(bytes);

    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let image = img.to_rgba8();
    debug!("Loaded {name} [PNG]: {}x{}", image.width(), image.height());
    Ok(Png {
        image,
        bit_depth,
        color_type,
    })
}

/// Reads `bit_depth` and `color_type` from the IHDR chunk that immediately
/// follows the 8-byte PNG signature. Returns sentinel zeros if the buffer
/// is too short — the caller will then fail when the actual decoder
/// rejects the malformed input, so we don't need to surface this here.
fn parse_ihdr(bytes: &[u8]) -> (u8, PngColorType) {
    // Signature (8) + chunk length (4) + "IHDR" (4) + width (4) + height (4)
    // = 24 bytes before bit_depth. The IHDR data section is always
    // exactly 13 bytes per the spec, so byte 24 is bit_depth and byte 25
    // is color_type whenever the file is even minimally well-formed.
    if bytes.len() < 26 || bytes[..8] != PNG_SIGNATURE {
        return (0, PngColorType::Other(0));
    }
    let bit_depth = bytes[24];
    let color_type = PngColorType::from_u8(bytes[25]);
    (bit_depth, color_type)
}

/// PNG IHDR color-type byte.
///
/// See PNG spec §11.2.2. Bit-depth restrictions for each variant are not
/// enforced here — we just round-trip the byte for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngColorType {
    /// Each pixel is a grayscale sample.
    Grayscale,
    /// Each pixel is an R, G, B triple.
    Rgb,
    /// Each pixel is a palette index; a `PLTE` chunk shall appear.
    Palette,
    /// Each pixel is a grayscale sample, followed by an alpha sample.
    GrayscaleAlpha,
    /// Each pixel is an R, G, B triple, followed by an alpha sample.
    Rgba,
    /// Anything the spec doesn't define (kept for forward-compat parsing).
    Other(u8),
}

impl PngColorType {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Grayscale,
            2 => Self::Rgb,
            3 => Self::Palette,
            4 => Self::GrayscaleAlpha,
            6 => Self::Rgba,
            n => Self::Other(n),
        }
    }
}

impl std::fmt::Display for PngColorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grayscale => f.write_str("Grayscale"),
            Self::Rgb => f.write_str("RGB"),
            Self::Palette => f.write_str("Palette"),
            Self::GrayscaleAlpha => f.write_str("Grayscale+Alpha"),
            Self::Rgba => f.write_str("RGBA"),
            Self::Other(n) => write!(f, "Unknown({n})"),
        }
    }
}

/// A decoded PNG file.
#[derive(Debug)]
pub struct Png {
    /// Decoded pixels, always upgraded to RGBA8 regardless of the source
    /// PNG's color type or bit depth.
    pub image: ImageBuffer<Rgba<u8>, Vec<u8>>,
    /// Original bit depth as declared in the IHDR chunk (1, 2, 4, 8, 16).
    pub bit_depth: u8,
    /// Color type as declared in the IHDR chunk.
    pub color_type: PngColorType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    #[test]
    fn test_parse_png_rgba() {
        // GUIWRLP8.png is a 6×600 RGBA PNG used as the MOSC test fixture.
        let path = get_assets_path().join("MOS/MOSC/GUIWRLP8.png");
        let data = DataSource::new(path.clone());
        let png = PngImporter { name: "png_test" }.import(&data).unwrap();

        let original = image::open(&path).unwrap();
        assert_images_are_equal(&png.image.clone().into(), &original, None);
        assert_eq!(png.image.width(), 6);
        assert_eq!(png.image.height(), 600);
    }

    #[test]
    fn test_metadata_rgba_8bpp() {
        let path = get_assets_path().join("MOS/MOSC/GUIWRLP8.png");
        let png = PngImporter { name: "png_test" }
            .import(&DataSource::new(path))
            .unwrap();
        assert_eq!(png.color_type, PngColorType::Rgba);
        assert_eq!(png.bit_depth, 8);
    }

    #[test]
    fn test_color_type_display() {
        assert_eq!(PngColorType::Rgba.to_string(), "RGBA");
        assert_eq!(PngColorType::Palette.to_string(), "Palette");
        assert_eq!(PngColorType::Other(7).to_string(), "Unknown(7)");
    }

    #[test]
    fn test_decode_rejects_non_png() {
        // BMP signature would crash the decoder; surface as InvalidData.
        let data = DataSource::new(get_assets_path().join("BMP/CCHAN05.BMP"));
        let err = PngImporter { name: "png_test" }.import(&data).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
