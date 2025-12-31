use std::io::{BufRead, Seek};

use image::{ImageBuffer, Rgba};

use crate::{datasource::{DataSource, Importer, Reader}, fs::{CaseInsensitiveFS, CaseInsensitivePath}, resource::{bam::Type, pvr::PvrzImporter}};

/// A BAM V2 file importer
pub struct BamV2Parser;

impl BamV2Parser {
    /// Imports a BAM V2 file
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<BamV2> {

        let signature = reader.read_string(8)?;
        let expected_type = Type::BamV2;

        if !signature.eq(expected_type.signature()) {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let frames_count = reader.read_u32()? as usize;
        let cycles_count = reader.read_u32()? as usize;
        let data_blocks_count = reader.read_u32()? as usize;

        let frames_offset = reader.read_u32()? as u64;
        let cycles_offset = reader.read_u32()? as u64;
        let data_blocks_offset = reader.read_u32()? as u64;

        // initializing frames
        let frames = {
            reader.set_position(frames_offset)?;
            let mut frames = Vec::with_capacity(frames_count);
            for _ in 0..frames_count {
                let width = reader.read_u16()? as u32;
                let height = reader.read_u16()? as u32;
                let center_x = reader.read_u16()? as u32;
                let center_y = reader.read_u16()? as u32;
                let data_blocks_start_index = reader.read_u16()? as usize;
                let data_blocks_count = reader.read_u16()? as usize;

                frames.push(BamV2Frame {
                    width,
                    height,
                    center_x,
                    center_y,
                    data_blocks_count,
                    data_blocks_start_index,
                });
            }

            frames
        };

        // initializing cycles
        let cycles = {
            reader.set_position(cycles_offset)?;
            let mut cycles = Vec::with_capacity(cycles_count);
            for _ in 0..cycles_count {
                let frames_count = reader.read_u16()? as usize;
                let frame_start_index = reader.read_u16()? as usize;

                cycles.push(BamV2Cycle {
                    frames_count,
                    frame_start_index,
                });
            }

            cycles
        };

        // initializing data blocks
        let data_blocks = {
            reader.set_position(data_blocks_offset)?;
            let mut data_blocks = Vec::with_capacity(data_blocks_count);
            for _ in 0..data_blocks_count {
                let pvrz_page = reader.read_u32()?;
                let source_x_coordinate = reader.read_u32()?;
                let source_y_coordinate = reader.read_u32()?;
                let width = reader.read_u32()?;
                let height = reader.read_u32()?;
                let target_x_coordinate = reader.read_u32()?;
                let target_y_coordinate = reader.read_u32()?;

                data_blocks.push(BamV2DataBlock {
                    pvrz_page,
                    width,
                    height,
                    source_x_coordinate,
                    source_y_coordinate,
                    target_x_coordinate,
                    target_y_coordinate,
                });
            }

            data_blocks
        };


        Ok(BamV2 {
            r#type: expected_type,
            frames,
            cycles,
            data_blocks,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV2 {
    /// The type of the file
    pub r#type: Type,
    /// The frames of the image
    pub frames: Vec<BamV2Frame>,
    /// The image cycles
    pub cycles: Vec<BamV2Cycle>,
    /// The data blocks
    pub data_blocks: Vec<BamV2DataBlock>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV2Cycle {
    /// Count of frame entries in this cycle
    pub frames_count: usize,
    /// Start index of frame entries in this cycle
    pub frame_start_index: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV2Frame {
    pub width: u32,
    pub height: u32,
    pub center_x: u32,
    pub center_y: u32,
    /// Count of data_block entries in this cycle
    pub data_blocks_count: usize,
    /// Start index of data_block entries in this cycle
    pub data_blocks_start_index: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BamV2DataBlock {
    // PVRZ page. Refers to MOSxxxx.PVRZ files, where xxxx is a zero-padded four-digits decimal number.
    pub pvrz_page: u32,
    pub width: u32,
    pub height: u32,
    pub source_x_coordinate: u32,
    pub source_y_coordinate: u32,
    pub target_x_coordinate: u32,
    pub target_y_coordinate: u32,
}

impl BamV2DataBlock {

    /// Returns the MOSxxxx.PVRZ files name associated with this data block
    pub fn pvrz_name(&self) -> String {
        format!("MOS{:04}.PVRZ", self.pvrz_page)
    }
}

impl BamV2 {
    
    /// Exports the frame to an image file.
    /// The image type is determined by the file extension.
    pub fn frame_to_image(&self, frame_index: usize, fs: &CaseInsensitiveFS) -> image::ImageResult<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        
        let frame = if let Some(frame) = self.frames.get(frame_index) {
            frame
        } else {
            return Err(std::io::Error::other(format!(
                "Frame {} not found.",
                frame_index
            )))?;
        };

        let data_blocks = &self.data_blocks[frame.data_blocks_start_index..frame.data_blocks_start_index + frame.data_blocks_count];

        let mut target_image = image::ImageBuffer::new(frame.width, frame.height);
        let target_image_buffer = target_image.as_mut();

        for block in data_blocks {
            let pvrz_path = fs.search_path_opt(&CaseInsensitivePath::new(&block.pvrz_name())).ok_or(std::io::Error::other(format!(
                "PVRZ file {} not found.",
                block.pvrz_name()
            )))?;

            let datasource = DataSource::new(pvrz_path);
            // Suboptimal: PVRZ images should be cached
            let source_header = PvrzImporter::import(&datasource).unwrap();
            let source_image = PvrzImporter::to_image(&source_header, &datasource).unwrap();
            let source_image_buffer = source_image.as_raw();

            for row in 0..block.height {
                let block_source_row = block.source_y_coordinate + row;
                let block_destination_row = block.target_y_coordinate + row;

                let source_start = ((block_source_row * source_header.width + block.source_x_coordinate) * 4) as usize;
                let source_end   = source_start + (block.width * 4) as usize;

                let target_start = ((block_destination_row * frame.width + block.target_x_coordinate) * 4) as usize;
                let target_end   = target_start + (block.width * 4) as usize;

                target_image_buffer[target_start..target_end]
                    .copy_from_slice(&source_image_buffer[source_start..source_end]);
            }
        }

        Ok(target_image)
    }

}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};

    use super::*;

    #[test]
    fn test_bam_v2_data_block_pvrz_name() {
        let mut data_block = BamV2DataBlock {
            pvrz_page: 1,
            width: 0,
            height: 0,
            source_x_coordinate: 0,
            source_y_coordinate: 0,
            target_x_coordinate: 0,
            target_y_coordinate: 0,
        };
        assert_eq!(data_block.pvrz_name(), "MOS0001.PVRZ");

        data_block.pvrz_page = 1234;
        assert_eq!(data_block.pvrz_name(), "MOS1234.PVRZ");
    }

        #[test]
    fn test_parse_bam_v2_should_fail_if_wrong_signature() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V1/01/1chan03B_compressed.BAM"
        )));

        let mut reader = data.reader().unwrap();
        let res = BamV2Parser::import(&mut reader);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_bam_v2_02() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}/resources/BAM_V2/02/1CHELM03.BAM"
        )));

        let mut reader = data.reader().unwrap();
        let bam = BamV2Parser::import(&mut reader).unwrap();

        assert_eq!(bam.r#type, Type::BamV2);

        assert_eq!(bam.cycles.len(), 1);
        assert_eq!(
            bam.cycles[0],
            BamV2Cycle {
                frames_count: 1,
                frame_start_index: 0
            }
        );

        assert_eq!(bam.frames.len(), 1);
        assert_eq!(bam.frames[0].width, 128);
        assert_eq!(bam.frames[0].height, 154);
        assert_eq!(bam.frames[0].center_x, 0);
        assert_eq!(bam.frames[0].center_y, 0);
        assert_eq!(bam.frames[0].data_blocks_start_index, 0);
        assert_eq!(bam.frames[0].data_blocks_count, 8);

        assert_eq!(bam.data_blocks.len(), 8);
        assert_eq!(bam.data_blocks[0].pvrz_page, 0);
        assert_eq!(bam.data_blocks[0].width, 128);
        assert_eq!(bam.data_blocks[0].height, 32);
        assert_eq!(bam.data_blocks[0].source_x_coordinate, 191);
        assert_eq!(bam.data_blocks[0].source_y_coordinate, 1);
        assert_eq!(bam.data_blocks[0].target_x_coordinate, 0);
        assert_eq!(bam.data_blocks[0].target_y_coordinate, 0);

        let TEST_DECODE_PVRZ_IMAGE = 0;

        let fs = CaseInsensitiveFS::new(format!("{RESOURCES_DIR}/resources/BAM_V2/02/")).unwrap();
        let image = bam.frame_to_image(0, &fs).unwrap();
        image.save("./test.png").unwrap();

    }

    
}

