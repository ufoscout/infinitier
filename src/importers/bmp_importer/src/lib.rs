use image::{ImageBuffer, Rgba};
use log::debug;

use infinitier_datasource::{DataSource, Importer};

/// A BMP file importer
pub struct BmpImporter;

impl Importer for BmpImporter {
    type T = Bmp;

    fn import(&self, source: &DataSource) -> std::io::Result<Bmp> {
        let mut reader = source.reader()?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        decode_bmp(&bytes)
    }
}

fn decode_bmp(bytes: &[u8]) -> std::io::Result<Bmp> {
    if let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp) {
        let image = img.to_rgba8();
        debug!("Loaded BMP: {}x{}", image.width(), image.height());
        return Ok(Bmp { image });
    }

    // Some BMP files (e.g. original BG) declare more palette entries than the
    // bit depth allows. Patch clr_used at header offset 46 to cap it at 2^bit_count.
    if bytes.len() >= 50 {
        let bit_count = u16::from_le_bytes([bytes[28], bytes[29]]);
        if bit_count > 0 && bit_count <= 8 {
            let max_colors = 1u32 << bit_count;
            let clr_used = u32::from_le_bytes(bytes[46..50].try_into().unwrap());
            if clr_used > max_colors {
                let mut patched = bytes.to_vec();
                patched[46..50].copy_from_slice(&max_colors.to_le_bytes());
                let image = image::load_from_memory_with_format(&patched, image::ImageFormat::Bmp)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                    .to_rgba8();
                debug!("Loaded BMP (patched palette {} -> {}): {}x{}", clr_used, max_colors, image.width(), image.height());
                return Ok(Bmp { image });
            }
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Failed to decode BMP",
    ))
}

/// A BMP file
#[derive(Debug)]
pub struct Bmp {
    pub image: ImageBuffer<Rgba<u8>, Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    #[test]
    fn test_parse_bmp_01() {
        let data = DataSource::new(get_assets_path().join("resources/BMP/CCHAN05.BMP"));

        let original = image::open(get_assets_path().join("resources/BMP/CCHAN05.BMP")).unwrap();

        let bmp = BmpImporter.import(&data).unwrap();

        assert_images_are_equal(&bmp.image.into(), &original);
    }

    #[test]
    fn test_parse_bmp_02() {
        let data = DataSource::new(get_assets_path().join("resources/BMP/MINSCM.BMP"));

        let original = image::open(get_assets_path().join("resources/BMP/MINSCM.BMP")).unwrap();

        let bmp = BmpImporter.import(&data).unwrap();

        assert_images_are_equal(&bmp.image.into(), &original);
    }
}
