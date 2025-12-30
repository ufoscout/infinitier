// To decode PVR texture files check: https://crates.io/crates/texture2ddecoder

use std::u64;

use image::{ImageBuffer, Rgba};

use crate::datasource::{DataSource, Importer};

/// A PRVZ file importer
pub struct PrvzImporter;

impl Importer for PrvzImporter {
    type T = Prvz;


    fn import(source: &DataSource) -> std::io::Result<Prvz> {

        let mut reader = source.reader()?;

        // header
        {
            let version = reader.read_u32()?;
            let flags = reader.read_u32()?;
            let pixel_format = reader.read_u64()?;
            let color_space = reader.read_u32()?;
            let channel_type = reader.read_u32()?;
            let height = reader.read_u32()?;
            let width = reader.read_u32()?;
            let depth = reader.read_u32()?;
            let surfaces_number = reader.read_u32()?;
            let faces_number = reader.read_u32()?;
            let mip_map_count = reader.read_u32()?;
            let metadata_size = reader.read_u32()?;

            println!("version: {}", version);
            println!("flags: {}", flags);
            println!("pixel_format: {}", pixel_format);
            println!("color_space: {}", color_space);
            println!("channel_type: {}", channel_type);
            println!("height: {}", height);
            println!("width: {}", width);
            println!("depth: {}", depth);
            println!("surfaces_number: {}", surfaces_number);
            println!("faces_number: {}", faces_number);
            println!("mip_map_count: {}", mip_map_count);
            println!("metadata_size: {}", metadata_size);
        }

        let mut data = vec![];
        reader.read_to_end(&mut data, u64::MAX).unwrap();
        let is2bpp = false;

        let width = 512u32;
        let height = 512u32;

        {
            
            let block_width: usize = if is2bpp { 8 } else { 4 };
            let num_blocks_x: usize = (width  as usize).div_ceil(block_width);
            let num_blocks_y: usize = (height  as usize).div_ceil(4);
            let num_blocks: usize = num_blocks_x * num_blocks_y;
            let min_num_blocks: usize = num_blocks_x.min(num_blocks_y);

            println!("num_blocks_x: {}", num_blocks_x);
            println!("num_blocks_y: {}", num_blocks_y);
            println!("num_blocks: {}", num_blocks);
            println!("min_num_blocks: {}", min_num_blocks);
            println!("data expected size: {}", num_blocks * block_width);
        }
        println!("size: {}", width * height);
        println!("data: {}", data.len());

        let mut image = vec![0u32; width as usize * height as usize];

        // decode DXT1 aka BC1
        // texture2ddecoder::decode_bc1(&data, width as usize, height as usize, &mut image).unwrap();

        // decode DXT5 aka BC3
        texture2ddecoder::decode_bc3(&data, width as usize, height as usize, &mut image).unwrap();

         let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_fn(width, height, |x, y| {
                let idx = (y * width + x) as usize;
                let p = image[idx];
                Rgba([
                    ((p >> 16) & 0xFF) as u8, // G
                    ((p >> 8)  & 0xFF) as u8, // B
                    (p & 0xFF) as u8,         // A
                    ((p >> 24) & 0xFF) as u8, // R
                ])
            });
        img.save("./prvz.png").unwrap();


        todo!();

    }

}

#[derive(Debug, PartialEq, Eq)]
pub struct Prvz {}


#[cfg(test)]
mod tests {

    use std::path::Path;

    use image::GenericImageView;
    use tempfile::TempDir;

    use super::*;
    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};

    #[test]
    fn test_parse_prvz() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/MOS_DXT5/MOS0000.PVR"
        )));

        let res = PrvzImporter::import(&data).unwrap();
        

    }

}
