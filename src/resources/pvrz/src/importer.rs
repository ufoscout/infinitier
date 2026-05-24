use std::io::Read;

use image::{ImageBuffer, Rgba};
use infinitier_datasource::{DataSource, Importer, ReadExt};
use log::{debug, error};

use crate::{PvrDataCompression, Pvrz, PvrzHeader};

/// A PVRZ file importer
pub struct PvrzImporter<'a> {
    pub name: &'a str,
}

impl Importer for PvrzImporter<'_> {
    type T = Pvrz;

    /// Imports a PVRZ file which is a PVR file with Zlib compression.
    ///
    /// PVRZ = a 4-byte length prefix (purpose unclear; gemrb treats it as
    /// an endianness marker when equal to `0x50565203`) followed by a
    /// zlib stream containing a 52-byte PVR header and a DXT-compressed
    /// pixel payload. Games based on the Infinity engine only use
    /// `pixel_format` 7 (DXT1/BC1) and 11 (DXT5/BC3).
    ///
    /// Header parsing and image decoding happen in a single pass over the
    /// zlib stream, so the returned [`Pvrz`] is fully self-contained.
    ///
    /// PVR file format reference:
    /// <https://docs.imgtec.com/specifications/pvr-file-format-specification/html/topics/pvr-introduction.html>
    fn import(&self, source: &DataSource) -> std::io::Result<Pvrz> {
        let mut reader = source.reader()?;

        // Discard the 4-byte length prefix.
        let _size = reader.read_u32()?;

        let mut reader = reader.as_zip_reader();

        let version = reader.read_u32()?;
        if version != 0x03525650 {
            error!("Invalid PVR signature: 0x{:08X}", version);
            return Err(std::io::Error::other(format!(
                "Invalid PVR signature: 0x{:08X}",
                version
            )));
        }
        let header = PvrzHeader {
            version,
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
        };

        // Skip the variable-length metadata block, then pull the rest of
        // the zlib stream — that's the DXT payload.
        if header.metadata_size > 0 {
            let mut skip = vec![0u8; header.metadata_size as usize];
            reader.read_exact(&mut skip)?;
        }

        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        let image = decode_pixels(&header, &data)?;

        debug!(
            "Loaded {} [PVRZ]: {}x{} {:?}",
            self.name, header.width, header.height, header.pixel_format
        );
        Ok(Pvrz { header, image })
    }
}

/// Decode the DXT payload into an RGBA8 [`ImageBuffer`].
fn decode_pixels(
    header: &PvrzHeader,
    data: &[u8],
) -> std::io::Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    let mut pixels = vec![0u32; header.width as usize * header.height as usize];
    match header.pixel_format {
        PvrDataCompression::DXT1 => {
            // decode DXT1 aka BC1
            texture2ddecoder::decode_bc1a(
                data,
                header.width as usize,
                header.height as usize,
                &mut pixels,
            )
            .map_err(std::io::Error::other)?;
        }
        PvrDataCompression::DXT5 => {
            // decode DXT5 aka BC3
            texture2ddecoder::decode_bc3(
                data,
                header.width as usize,
                header.height as usize,
                &mut pixels,
            )
            .map_err(std::io::Error::other)?;
        }
    }

    Ok(ImageBuffer::from_fn(header.width, header.height, |x, y| {
        let idx = (y * header.width + x) as usize;
        let p = pixels[idx];
        Rgba([
            ((p >> 16) & 0xFF) as u8, // R
            ((p >> 8) & 0xFF) as u8,  // G
            (p & 0xFF) as u8,         // B
            ((p >> 24) & 0xFF) as u8, // A
        ])
    }))
}

#[cfg(test)]
mod tests {

    use super::*;
    use infinitier_test_utils::{assert_images_are_equal, get_assets_path};

    #[test]
    fn test_parse_pvrz_dxt1() {
        let data = DataSource::new(get_assets_path().join("PVRZ/DXT1/A004602.PVRZ"));

        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        assert_eq!(
            pvrz.header,
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

        assert_images_are_equal(
            &image::open(get_assets_path().join("PVRZ/DXT1/A004602.PNG")).unwrap(),
            &pvrz.image.into(),
            None,
        );
    }

    #[test]
    fn test_parse_pvrz_dxt5() {
        let data = DataSource::new(get_assets_path().join("PVRZ/DXT5/MOS0000.PVRZ"));

        let pvrz = PvrzImporter { name: "pvrz_test" }.import(&data).unwrap();

        assert_eq!(
            pvrz.header,
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

        assert_images_are_equal(
            &image::open(get_assets_path().join("PVRZ/DXT5/MOS0000.PNG")).unwrap(),
            &pvrz.image.into(),
            None,
        );
    }
}
