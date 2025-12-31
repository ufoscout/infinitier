// To decode PVR texture files check: https://crates.io/crates/texture2ddecoder

use crate::datasource::{DataSource, Importer};
use image::{ImageBuffer, Rgba};

/// A PVRZ file importer
pub struct PvrzImporter;

impl Importer for PvrzImporter {
    type T = PvrzHeader;

    /// Imports a PVRZ file which is a PVR file with Zlib compression.
    /// For the PVR file format see: https://docs.imgtec.com/specifications/pvr-file-format-specification/html/topics/pvr-introduction.html
    /// PVR is essentially a texture container that contains a header and data is compressed by a DDS algorithm
    /// (see: https://crates.io/crates/dds).
    /// The specific algorithm for the data is speficified in the PVR pixel_format header field. Games based on the Infinity engine
    /// only use pixel_format 7 (DXT1/BC1) and 11 (DXT5/BC3).
    fn import(source: &DataSource) -> std::io::Result<PvrzHeader> {
        let mut reader = source.reader()?;

        // Not sure for what this is used.
        // gemrb use BigEndianess if this value is equal to 0x50565203.
        let _size = reader.read_u32()?;

        let mut reader = reader.as_zip_reader();

        // header
        Ok(PvrzHeader {
            version: reader.read_u32()?,
            flags: reader.read_u32()?,
            pixel_format: PvrDataCompression::from_u64(reader.read_u64()?)?,
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

impl PvrzImporter {
    /// Converts a PVRZ file to an image
    pub fn to_image(
        header: &PvrzHeader,
        source: &DataSource,
    ) -> image::ImageResult<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let mut reader = source.reader()?;
        // Not sure for what this is used.
        // gemrb use BigEndianess if this value is equal to 0x50565203.
        let _size = reader.read_u32()?;

        let mut reader = reader.as_zip_reader();

        // 52 is the size of the header
        reader.skip(52 + header.metadata_size as u64)?;

        let mut data = vec![];
        reader.read_to_end(&mut data, u64::MAX)?;

        let mut image = vec![0u32; header.width as usize * header.height as usize];

        match header.pixel_format {
            PvrDataCompression::DXT1 => {
                // decode DXT1 aka BC1
                texture2ddecoder::decode_bc1a(
                    &data,
                    header.width as usize,
                    header.height as usize,
                    &mut image,
                )
                .map_err(std::io::Error::other)?;
            }
            PvrDataCompression::DXT5 => {
                // decode DXT5 aka BC3
                texture2ddecoder::decode_bc3(
                    &data,
                    header.width as usize,
                    header.height as usize,
                    &mut image,
                )
                .map_err(std::io::Error::other)?;
            }
        }

        Ok(ImageBuffer::from_fn(header.width, header.height, |x, y| {
            let idx = (y * header.width + x) as usize;
            let p = image[idx];
            Rgba([
                ((p >> 16) & 0xFF) as u8, // R
                ((p >> 8) & 0xFF) as u8,  // G
                (p & 0xFF) as u8,         // B
                ((p >> 24) & 0xFF) as u8, // A
            ])
        }))
    }
}

/// A PVR header
#[derive(Debug, PartialEq, Eq)]
pub struct PvrzHeader {
    pub version: u32,
    pub flags: u32,
    pub pixel_format: PvrDataCompression,
    pub color_space: u32,
    pub channel_type: u32,
    pub height: u32,
    pub width: u32,
    pub depth: u32,
    pub surfaces_number: u32,
    pub faces_number: u32,
    pub mip_map_count: u32,
    pub metadata_size: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PvrDataCompression {
    /// DXT1 aka BC1 compressed texture
    DXT1,
    /// DXT5 aka BC3 compressed texture
    DXT5,
}

impl PvrDataCompression {
    /// Converts a u64 value to a `PvrDataCompression` enum variant.
    pub fn from_u64(value: u64) -> std::io::Result<PvrDataCompression> {
        match value {
            7 => Ok(PvrDataCompression::DXT1),
            11 => Ok(PvrDataCompression::DXT5),
            _ => Err(std::io::Error::other(format!(
                "Unexpected pixel_format: {}",
                value
            ))),
        }
    }

    /// Converts a `PvrDataCompression` enum variant to a u32 value
    pub fn to_u64(&self) -> u64 {
        match self {
            PvrDataCompression::DXT1 => 7,
            PvrDataCompression::DXT5 => 11,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        datasource::DataSource, resource::test_utils::assert_images_are_equal,
        test_utils::RESOURCES_DIR,
    };
    use std::path::Path;

    #[test]
    fn test_parse_pvrz_dxt1() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/MOS_DXT1/A004602.PVRZ"
        )));

        let pvrz_header = PvrzImporter::import(&data).unwrap();

        assert_eq!(
            pvrz_header,
            PvrzHeader {
                version: 55727696,
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
                metadata_size: 0
            }
        );

        // Assert that the image is the same as the reference
        {
            let image = PvrzImporter::to_image(&pvrz_header, &data).unwrap();

            assert_images_are_equal(
                &image::open(Path::new(&format!(
                    "{RESOURCES_DIR}/resources/MOS_DXT1/A004602.PNG"
                )))
                .unwrap(),
                &image.into(),
            );
        }
    }

    #[test]
    fn test_parse_pvrz_dxt5() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/MOS_DXT5/MOS0000.PVRZ"
        )));

        let pvrz_header = PvrzImporter::import(&data).unwrap();

        assert_eq!(
            pvrz_header,
            PvrzHeader {
                version: 55727696,
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
                metadata_size: 0
            }
        );

        // Assert that the image is the same as the reference
        {
            let image = PvrzImporter::to_image(&pvrz_header, &data).unwrap();

            assert_images_are_equal(
                &image::open(Path::new(&format!(
                    "{RESOURCES_DIR}/resources/MOS_DXT5/MOS0000.PNG"
                )))
                .unwrap(),
                &image.into(),
            );
        }
    }
}
