use image::{ImageBuffer, Rgba};

use crate::datasource::DataSource;

/// A BMP file importer
pub struct BmpImporter;

impl BmpImporter {
    pub fn to_image(source: &DataSource) -> image::ImageResult<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let reader = source.reader()?;
        Ok(image::ImageReader::with_format(reader.data, image::ImageFormat::Bmp)
            .decode()?.to_rgba8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use crate::{
        datasource::DataSource, resource::test_utils::assert_images_are_equal,
        test_utils::RESOURCES_DIR,
    };

    #[test]
    fn test_parse_bmp_01() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BMP/CCHAN05.BMP"
        )));

        let original = image::open(Path::new(&format!(
                    "{RESOURCES_DIR}/resources/BMP/CCHAN05.BMP"
                )))
                .unwrap();

        let image = BmpImporter::to_image(&data).unwrap();

        assert_images_are_equal(&image.into(), &original);
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

        let image = BmpImporter::to_image(&data).unwrap();

        assert_images_are_equal(&image.into(), &original);
    }
}