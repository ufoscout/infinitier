use std::{io::{BufRead, Seek}, path::Path};

use image::{ImageBuffer, Rgba};

use crate::{
    datasource::Reader,
    resource::bam::Type,
};

#[derive(Debug, PartialEq, Eq)]
pub struct BamV1 {
    /// The type of the file
    pub r#type: Type,
    /// The frames of the image
    pub frames: Vec<BamV1Frame>,
    /// The colors palette
    pub palette: Vec<Rgb>,
    /// The imagecycles
    pub cycles: Vec<BamV1Cycle>,
    /// The index of the RLE compressed color in the palette
    pub rle_compressed_color_index: u8,

}

/// A BAM V1 file importer
pub struct BamV1Parser;

impl BamV1Parser {
    /// Imports a BAM V1 file
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<BamV1> {
        let signature = reader.read_string(8)?;
        let expected_type = Type::BamV1;

        if !signature.eq(expected_type.signature()) {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let frames_count = reader.read_u16()? as usize;
        let cycles_count = reader.read_u8()? as usize;
        let rle_compressed_color_index = reader.read_u8()?;

        let frames_offset = reader.read_u32()? as u64;
        let palette_offset = reader.read_u32()? as u64;
        let lookup_offset = reader.read_u32()? as u64;

        // Initializing palette
        let palette = {
            let palette_entries = 256.min((lookup_offset - palette_offset) / 4) as usize;
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

                palette.push(Rgb { r, g, b, alpha });
            }

            let _ = std::mem::replace(
                &mut palette[transparency_index],
                Rgb {
                    r: 0,
                    g: 255,
                    b: 0,
                    alpha: 0,
                },
            );

            palette
        };

        // initializing frames
        let frames = {
            reader.set_position(frames_offset)?;
            let mut frames = Vec::with_capacity(frames_count);
            for _ in 0..frames_count {
                let width = reader.read_u16()? as u32;
                let height = reader.read_u16()? as u32;
                let center_x = reader.read_u16()?;
                let center_y = reader.read_u16()?;
                let data_bits = reader.read_u32()?;
                let data_offset = (data_bits & 0x7fffffff) as u64;
                let compressed = (data_bits & 0x80000000) == 0;

                let size = (width * height) as usize;
                let position = reader.position()?;

                let mut pixel_palette_indexes = Vec::with_capacity(size);
                reader.set_position(data_offset)?;
                while pixel_palette_indexes.len() < size {

                    let pixel_index = reader.read_u8()?;

                    if compressed && (pixel_index == rle_compressed_color_index) {
                        let pixels_count = reader.read_u8()?;
                        for _ in 0..=pixels_count {
                            pixel_palette_indexes.push(pixel_index);
                        }
                    } else {
                        pixel_palette_indexes.push(pixel_index);
                    }

                }

                reader.set_position(position)?;

                frames.push(BamV1Frame {
                    width,
                    height,
                    center_x,
                    center_y,
                    pixel_palette_indexes,
                });
            }

            frames
        };

        // initializing cycles
        let cycles = {
            let mut cycles = Vec::with_capacity(cycles_count);
            for _ in 0..cycles_count {
                // number of frame indices in this cycle
                let indices_count = reader.read_u16()? as usize;
                // Index into frame lookup table of first frame in this cycle
                let lookup_table_index = reader.read_u16()? as u64;

                let position = reader.position()?;

                // list of frame indices used in this cycle
                let mut frame_indices = Vec::with_capacity(indices_count);
                reader.set_position(lookup_offset + (2 * lookup_table_index))?;
                for _ in 0..indices_count {
                    let frame_index = reader.read_u16()?;
                    frame_indices.push(frame_index as usize);
                }

                cycles.push(BamV1Cycle {
                    frame_indices,
                });

                reader.set_position(position)?;
            }
           cycles     
        };

        // for frame in frames {
        //     reader.set_position(frame.data_offset)?;

        //     let mut frame_colors = Vec::with_capacity((frame.width * frame.height) as usize);
        //     while frame_colors.len() < (frame.width * frame.height) as usize {

        //         let pixel_index = reader.read_u8()?;
        //         let color = &palette[pixel_index as usize];

        //         if frame.compressed && (pixel_index == rle_compressed_color_index) {
        //             let pixels_count = reader.read_u8()?;
        //             for _ in 0..=pixels_count {
        //                 frame_colors.push(color);
        //             }
        //         } else {
        //             frame_colors.push(color);
        //         }

        //     }

        //     save_png_rgba("./test.png", &frame_colors, frame.width, frame.height).unwrap();
        // }

        Ok(BamV1 {
            r#type: expected_type,
            frames,
            cycles,
            palette,
            rle_compressed_color_index
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub alpha: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV1Cycle {
    pub frame_indices: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV1Frame {
    pub width: u32,
    pub height: u32,
    pub center_x: u16,
    pub center_y: u16,
    /// The indexes of the pixels in the palette
    pub pixel_palette_indexes: Vec<u8>,
}

impl BamV1Frame {
    
    /// Exports the frame to a PNG file
    pub fn export_png<Q: AsRef<Path>>(&self, path: Q, palette: &[Rgb]) -> image::ImageResult<()> {
        let mut frame_colors = Vec::with_capacity((self.width * self.height) as usize);

        for pixel_index in &self.pixel_palette_indexes {
            let color = &palette[*pixel_index as usize];
            frame_colors.push(color);
        }

        let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_fn(self.width, self.height, |x, y| {
            let idx = (y * self.width + x) as usize;
            let p = frame_colors[idx];
            Rgba([p.r, p.g, p.b, p.alpha])
        });

        img.save(path)
    }
}

#[cfg(test)]
mod tests {

    use std::path::Path;

    use image::GenericImageView;
    use tempfile::TempDir;

    use super::*;
    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};

    #[test]
    fn test_detect_bam_v1_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V1/1chan03B_decompressed.BAM"
        )));

        let mut reader = data.reader().unwrap();
        let bam = BamV1Parser::import(&mut reader).unwrap();

        assert_eq!(bam.r#type, Type::BamV1);

        let tmp_dir = TempDir::new().unwrap();
        let path = tmp_dir.path().join("test.png");
        bam.frames[0].export_png(&path, &bam.palette).unwrap();

        assert_png_images_are_equal(
            Path::new(&format!(
                "{RESOURCES_DIR}/resources/BAM_V1/1chan03B00000.PNG"
            )),
            &path,
        );
    }


    /// Asserts that two png images are equal
    pub fn assert_png_images_are_equal<A: AsRef<Path>, B: AsRef<Path>>(path_a: A, path_b: B) {
        let img_a = image::open(path_a).unwrap();
        let img_b = image::open(path_b).unwrap();

        if img_a.dimensions() != img_b.dimensions() {
            panic!("Images dimensions are different");
        }

        if img_a.color() != img_b.color() {
            panic!("Images colors are different");
        }

        if img_a.to_rgba8() != img_b.to_rgba8() {
            panic!("Images bytes are different");
        }

    }
}
