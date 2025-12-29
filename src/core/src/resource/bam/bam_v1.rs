use std::io::{BufRead, Seek};

use crate::{datasource::Reader, resource::bam::{Bam, Type}};

/// A BAM V1 file importer
pub struct BamV1Parser;

impl BamV1Parser {
    /// Imports a BAM V1 file
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<Bam> {
        let signature = reader.read_string(8)?;
        let expected_type = Type::BamV1;

        if !signature.eq(expected_type.signature()) {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let frames_count = reader.read_u16()? as usize;
        let cycles_count = reader.read_u8()? & 0xff;
        let rle_compressed_color_index = reader.read_u8()? & 0xff;

        let frames_offset = reader.read_u32()? as u64;
        let palette_offset = reader.read_u32()? as u64;
        let lookup_offset = reader.read_u32()? as u64;

        println!(
            "frames_count: {}, cycles_count: {}, rle_compressed_color_index: {}, frames_offset: {}, palette_offset: {}, lookup_offset: {}",
            frames_count, cycles_count, rle_compressed_color_index, frames_offset, palette_offset, lookup_offset
        );

        // initializing frames
        reader.set_position(frames_offset)?;
        let mut frames = Vec::with_capacity(frames_count);
        for _ in 0..frames_count {
            let width = (reader.read_u16()? & 0xffff) as u32;
            let height = (reader.read_u16()? & 0xffff) as u32;
            let center_x = reader.read_u16()?;
            let center_y = reader.read_u16()?;
            let data_bits = reader.read_u32()?;
            let data_offset = (data_bits & 0x7fffffff) as u64;
            let compressed = (data_bits & 0x80000000) == 0;

            frames.push(Frame {
                width,
                height,
                center_x,
                center_y,
                data_bits,
                data_offset,
                compressed,
            });

            println!(
                "width: {}, height: {}, center_x: {}, center_y: {}, data_bits: {}, data_offset: {}, compressed: {}",
                width, height, center_x, center_y, data_bits, data_offset, compressed
            );
        }

        // initializing cycles
        for _ in 0..cycles_count {
            // number of frame indices in this cycle
            let indices_count = (reader.read_u16()? & 0xffff) as usize;
            // Index into frame lookup table of first frame in this cycle
            let lookup_table_index = (reader.read_u16()? & 0xffff) as u64;

            
            let position = reader.position()?;
            
            // list of frame indices used in this cycle
            let mut frame_indices = Vec::with_capacity(indices_count);
            reader.set_position(lookup_offset + (2*lookup_table_index))?;
            for _ in 0..indices_count {
                let frame_index = reader.read_u16()?;
                frame_indices.push(frame_index);
            }
            
            println!(
                "indices_count: {}, lookup_table_index: {}, frame_indices: {:?}",
                indices_count, lookup_table_index, frame_indices
            );


            reader.set_position(position)?;
        }

        let palette_entries = 256.min((lookup_offset - palette_offset) / 4)as usize;
        let mut palette = Vec::with_capacity(palette_entries);
        reader.set_position(palette_offset)?;

        let mut transparency_index = 0;

        for i in 0..palette_entries {
            let b = reader.read_u8()?;
            let g = reader.read_u8()?;
            let r = reader.read_u8()?;
            let alpha = match reader.read_u8()? {
                0 => 255, // BAM in EE supports alpha, but for backwards compatibility an alpha of 0 is still 255
                x => x, // Alpha values of 01h .. FFh indicate transparency ranging from almost completely transparent to fully opaque. Full transparency can be realized by using palette index 0.  
            };

            // The transparency index is set to the first occurence of RGB(0,255,0). 
            // If RGB(0,255,0) does not exist in the palette then transparency index is set to 0
            if transparency_index == 0 && r == 0 && g == 255 && b == 0 {
                transparency_index = i;
            }

            // println!("r: {}, g: {}, b: {}, alpha: {}", r, g, b, alpha);
            palette.push( RGB { r, g, b, alpha });
        }

        let _ = std::mem::replace(&mut palette[transparency_index], RGB { r: 0, g: 0, b: 0, alpha: 0 });

        for frame in frames {
            reader.set_position(frame.data_offset)?;

            let mut frame_colors = Vec::with_capacity((frame.width * frame.height) as usize);
            while frame_colors.len() < (frame.width * frame.height) as usize{
                // println!("frame_colors.len(): {}", frame_colors.len());
                let pixel_index = reader.read_u8()?;
                let color = &palette[pixel_index as usize];
                // println!("pixel_index: {}, color: {:?}", pixel_index, color);

                if frame.compressed && (pixel_index == rle_compressed_color_index) {
                    // println!("rle_transparency_index: {}", rle_transparency_index);
                    let pixels_count = reader.read_u8()?;
                    for _ in 0..=pixels_count {
                        frame_colors.push(color);
                    }
                } else {
                    frame_colors.push(color);
                }

                // frame_colors.push(reader.read_u8()?);
            }

            println!("last frame_colors: {:?}", frame_colors.last().as_ref().unwrap());
            // println!("frame_data: {:?}, width: {}, height: {}", frame_data, frame.width, frame.height);
            save_png_rgba("./test.png", &frame_colors, frame.width, frame.height).unwrap();
        }


        Ok(Bam {
            r#type: expected_type,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RGB {
    r: u8,
    g: u8,
    b: u8,
    alpha: u8
}

#[derive(Debug, PartialEq, Eq)]
pub struct Frame {
                width: u32,
                height: u32,
                center_x: u16,
                center_y: u16,
                data_bits: u32,
                data_offset: u64,
                compressed: bool,
            }

use image::{ImageBuffer, Rgba};

pub fn save_png_rgba(
    path: &str,
    pixels: &[&RGB],
    width: u32,
    height: u32,
) -> image::ImageResult<()> {
    assert_eq!(pixels.len(), (width * height) as usize);

    let img: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_fn(width, height, |x, y| {
            let idx = (y * width + x) as usize;
            let p = &pixels[idx];
            Rgba([p.r, p.g, p.b, p.alpha])
        });

    img.save(path)
}

#[cfg(test)]
mod tests {

    use std::path::Path;

    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};
    use super::*;

    #[test]
    fn test_detect_bam_v1_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V1/1chan03B_decompressed.BAM"
        )));

        let mut reader = data.reader().unwrap();
        let bam = BamV1Parser::import(&mut reader).unwrap();

        assert_eq!(bam.r#type, Type::BamV1);
    }

}
    