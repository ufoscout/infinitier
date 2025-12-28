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

        let frames_count = reader.read_u16()?;
        let cycles_count = reader.read_u8()? & 0xff;
        let rle_transparency_index = reader.read_u8()? & 0xff;

        let frames_offset = reader.read_u32()? as u64;
        let palette_offset = reader.read_u32()? as u64;
        let lookup_offset = reader.read_u32()? as u64;

        println!(
            "frames_count: {}, cycles_count: {}, rle_transparency_index: {}, frames_offset: {}, palette_offset: {}, lookup_offset: {}",
            frames_count, cycles_count, rle_transparency_index, frames_offset, palette_offset, lookup_offset
        );

        reader.set_position(frames_offset)?;
        for _ in 0..frames_count {
            let width = reader.read_u16()? & 0xffff;
            let height = reader.read_u16()? & 0xffff;
            let center_x = reader.read_u16()?;
            let center_y = reader.read_u16()?;
            let data_bits = reader.read_u32()?;
            let data_offset = data_bits & 0x7fffffff;
            let compressed = (data_bits & 0x80000000) == 0;

            println!(
                "width: {}, height: {}, center_x: {}, center_y: {}, data_bits: {}, data_offset: {}, compressed: {}",
                width, height, center_x, center_y, data_bits, data_offset, compressed
            );
        }


        reader.set_position(palette_offset)?;


        reader.set_position(lookup_offset)?;


        Ok(Bam {
            r#type: expected_type,
        })
    }
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
    