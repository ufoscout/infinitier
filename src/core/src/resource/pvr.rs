// To decode PVR texture files check: https://crates.io/crates/texture2ddecoder

use std::{path::Path, u64};

use image::{ImageBuffer, Rgba};

use crate::datasource::{DataSource, Importer};

/// A PRVZ file importer
pub struct PrvzImporter;

impl Importer for PrvzImporter {
    type T = PrvzHeader;

    /// Imports a PRVZ file which is a PVR file with Zlib compression.
    /// For the PVR file format see: https://docs.imgtec.com/specifications/pvr-file-format-specification/html/topics/pvr-introduction.html
    /// PVR is essentially a texture container that contains a header and data is compressed by a DDS algorithm
    /// (see: https://crates.io/crates/dds).
    /// The specific algorithm for the data is speficified in the PRV pixel_format header field. Games based on the Infinity engine
    /// only use pixel_format 7 (DXT1/BC1) and 11 (DXT5/BC3).
    fn import(source: &DataSource) -> std::io::Result<PrvzHeader> {

        let mut reader = source.reader()?;

        // Not sure for what this is used.
        // gemrb use BigEndianess if this value is equal to 0x50565203.
        let _size = reader.read_u32()?;

        let mut reader = reader.as_zip_reader();

        // header
        Ok(PrvzHeader {
                version: reader.read_u32()?,
                flags: reader.read_u32()?,
                pixel_format: PrvDataCompression::from_u64(reader.read_u64()?)?,
                color_space: reader.read_u32()?,
                channel_type: reader.read_u32()?,
                height: reader.read_u32()?,
                width: reader.read_u32()?,
                depth: reader.read_u32()?,
                surfaces_number: reader.read_u32()?,
                faces_number: reader.read_u32()?,
                mip_map_count: reader.read_u32()?,
                metadata_size: reader.read_u32()?,
            })

    }

}

impl PrvzImporter {

    /// Exports a PRVZ file to an image file
    pub fn export_image<Q: AsRef<Path>>(path: Q, header: &PrvzHeader, source: &DataSource) -> image::ImageResult<()> {

        let mut reader = source.reader()?;
        // Not sure for what this is used.
        // gemrb use BigEndianess if this value is equal to 0x50565203.
        let _size = reader.read_u32()?;

        let mut reader = reader.as_zip_reader();

        // 52 is the size of the header
        reader.skip(52 + header.metadata_size as u64).unwrap();

        let mut data = vec![];
        reader.read_to_end(&mut data, u64::MAX).unwrap();

        let mut image = vec![0u32; header.width as usize * header.height as usize];

        match header.pixel_format {
            PrvDataCompression::DXT1 => {
                // decode DXT1 aka BC1
                texture2ddecoder::decode_bc1(&data, header.width as usize, header.height as usize, &mut image).unwrap();
            }
            PrvDataCompression::DXT5 => {
                // decode DXT5 aka BC3
                texture2ddecoder::decode_bc3(&data, header.width as usize, header.height as usize, &mut image).unwrap();
            }
        }

         let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_fn(header.width, header.height, |x, y| {
                let idx = (y * header.width + x) as usize;
                let p = image[idx];
                Rgba([
                    ((p >> 16) & 0xFF) as u8, // R
                    ((p >> 8)  & 0xFF) as u8, // G
                    (p & 0xFF) as u8,         // B
                    ((p >> 24) & 0xFF) as u8, // A
                ])
            });

            img.save(path)
    }
}

/// A PRV header
#[derive(Debug, PartialEq, Eq)]
pub struct PrvzHeader {
    pub version: u32,
    pub flags: u32,
    pub pixel_format: PrvDataCompression,
    pub color_space: u32,
    pub channel_type: u32,
    pub height: u32,
    pub width: u32,
    pub depth: u32,
    pub surfaces_number: u32,
    pub faces_number: u32,
    pub mip_map_count: u32,
    pub metadata_size: u32
}

#[derive(Debug, PartialEq, Eq)]
pub enum PrvDataCompression {
    /// DXT1 aka BC1 compressed texture 
    DXT1,
    /// DXT5 aka BC3 compressed texture
    DXT5
}

impl PrvDataCompression {
    
    /// Converts a u64 value to a `PrvDataCompression` enum variant.
    pub fn from_u64(value: u64) -> std::io::Result<PrvDataCompression> {
        match value {
            7 => Ok(PrvDataCompression::DXT1),
            11 => Ok(PrvDataCompression::DXT5),
            _ => Err(std::io::Error::other(format!(
                "Unexpected pixel_format: {}",
                value
            ))),
        }
    }

    /// Converts a `PrvDataCompression` enum variant to a u32 value
    pub fn to_u64(&self) -> u64 {
        match self {
            PrvDataCompression::DXT1 => 7,
            PrvDataCompression::DXT5 => 11,
        }
    }

}


#[cfg(test)]
mod tests {

    use std::path::Path;

    use image::GenericImageView;
    use tempfile::TempDir;

    use super::*;
    use crate::{datasource::DataSource, resource::test_utils::assert_png_images_are_equal, test_utils::RESOURCES_DIR};

    #[test]
    fn test_parse_prvz_dxt1() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/MOS_DXT1/A150024.PVRZ"
        )));

        let prvz_header = PrvzImporter::import(&data).unwrap();

        assert_eq!(prvz_header, PrvzHeader {
            version: 55727696,
            flags: 0,
            pixel_format: PrvDataCompression::DXT1,
            color_space: 0,
            channel_type: 0,
            height: 1024,
            width: 64,
            depth: 1,
            surfaces_number: 1,
            faces_number: 1,
            mip_map_count: 1,
            metadata_size: 0
        });

        // Assert that the image is the same as the reference
        {
            let tmp_dir = TempDir::new().unwrap();
            let path = tmp_dir.path().join("test.png");
            PrvzImporter::export_image("./test.png", &prvz_header, &data).unwrap();

            assert_png_images_are_equal(
                Path::new(&format!(
                    "{RESOURCES_DIR}/resources/MOS_DXT1/A150024.PNG"
                )),
                &path,
            );
        }

    }

    #[test]
    fn test_parse_prvz_dxt5() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/MOS_DXT5/MOS0000.PVRZ"
        )));

        let prvz_header = PrvzImporter::import(&data).unwrap();

        assert_eq!(prvz_header, PrvzHeader {
            version: 55727696,
            flags: 0,
            pixel_format: PrvDataCompression::DXT5,
            color_space: 0,
            channel_type: 0,
            height: 512,
            width: 512,
            depth: 1,
            surfaces_number: 1,
            faces_number: 1,
            mip_map_count: 1,
            metadata_size: 0
        });

        // Assert that the image is the same as the reference
        {
            let tmp_dir = TempDir::new().unwrap();
            let path = tmp_dir.path().join("test.png");
            PrvzImporter::export_image(&path, &prvz_header, &data).unwrap();

            assert_png_images_are_equal(
                Path::new(&format!(
                    "{RESOURCES_DIR}/resources/MOS_DXT5/MOS0000.PNG"
                )),
                &path,
            );
        }

    }

}
