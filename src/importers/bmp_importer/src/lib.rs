use image::{ImageBuffer, Rgba};
use log::debug;

use infinitier_datasource::{DataSource, Importer};

/// A BMP file importer
pub struct BmpImporter;

impl Importer for BmpImporter {
    type T = Bmp;

    fn import(&self, source: &DataSource) -> std::io::Result<Bmp> {
        let reader = source.reader()?;
        let image = image::ImageReader::with_format(reader.data, image::ImageFormat::Bmp)
            .decode()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?
            .to_rgba8();

        debug!("Loaded BMP: {}x{}", image.width(), image.height());
        Ok(Bmp { image })
    }
}

/// A BMP file
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
