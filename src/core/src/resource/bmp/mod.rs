use image::{ImageBuffer, Rgba};

use infinitier_datasource::{DataSource, Importer};

/// A BMP file importer
pub struct BmpImporter;

impl Importer for BmpImporter {
    type T = Bmp;

    fn import(source: &DataSource) -> std::io::Result<Bmp> {
        let reader = source.reader()?;
        let image = image::ImageReader::with_format(reader.data, image::ImageFormat::Bmp)
            .decode()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?
            .to_rgba8();

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
    use crate::{resource::test_utils::assert_images_are_equal, test_utils::RESOURCES_DIR};
    use std::path::Path;

    #[test]
    fn test_parse_bmp_01() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BMP/CCHAN05.BMP"
        )));

        let original = image::open(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BMP/CCHAN05.BMP"
        )))
        .unwrap();

        let bmp = BmpImporter::import(&data).unwrap();

        assert_images_are_equal(&bmp.image.into(), &original);
    }

    #[test]
    fn test_parse_bmp_02() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BMP/MINSCM.BMP"
        )));

        let original = image::open(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BMP/MINSCM.BMP"
        )))
        .unwrap();

        let bmp = BmpImporter::import(&data).unwrap();

        assert_images_are_equal(&bmp.image.into(), &original);
    }
}
