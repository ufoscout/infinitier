use std::{collections::VecDeque, io::{BufRead, Read}};

use encoding_rs::WINDOWS_1252;
use flate2::bufread::ZlibDecoder;

use crate::{datasource::{Reader, ReaderTrait, ZipReader}, resource::bif::{Bif, Type}};

/// A BIFC V1.0 file importer
pub struct BifcParser;

impl BifcParser {

    /// Imports a BIFC V1.0 file
    pub fn import<'a: 'b, 'b>(reader: &'b mut Reader<'a>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq("BIFCV1.0") {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        };

        let uncompressed_size = reader.read_u32()?;

        let files_number= 0;
        let tilesets_number = 0;

        {
            let mut compressed_reader = BifcCompressedReader{
                reader,
                offset: 0,
                buffer: VecDeque::new()
            };

            let mut buf = [0u8; 4];
            compressed_reader.read_exact(&mut buf)?;
            let uncompressed_size = u32::from_le_bytes(buf);

            let mut buf = [0u8; 4];
            compressed_reader.read_exact(&mut buf)?;
            let compressed_size = u32::from_le_bytes(buf);

            
            compressed_reader.read(&mut buf).unwrap();
        }

        let mut bif = Bif {
            r#type: Type::Bifc,
            files: Vec::with_capacity(files_number),
            tilesets: Vec::with_capacity(tilesets_number),
        };

        Ok(bif)
    }
}


struct BifcCompressedReader<'a: 'b, 'b>{
    reader: &'b mut Reader<'a>,
    offset: u64,
    buffer: VecDeque<u8>
}

impl <'a: 'b, 'b> Read for BifcCompressedReader<'a, 'b> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();

        if self.buffer.len() < len {
            self.fill_buffer()?;
        }

        let len = std::cmp::min(len, self.buffer.len());
        // This is probably inefficient. We only care that it works for now
        // for i in 0..len {
        //     buf[i] = self.buffer.pop_front().unwrap();
        // }
        self.buffer.read(buf)?;

        Ok(len)
    }
}

impl <'a: 'b, 'b> BifcCompressedReader<'a, 'b> {
    fn fill_buffer(&mut self) -> std::io::Result<usize> {

        let uncompressed_size = self.reader.read_u32()? as u64;
        let compressed_size = self.reader.read_u32()?;

        let mut reader = self.reader.as_zip_reader();

        // Inefficient but works for now
        let data = reader.take_to_vec(uncompressed_size)?;
        
        self.buffer = VecDeque::from(data);

        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};
    use super::*;


        #[test]
    fn test_detect_bifc_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}bg2/data/Data/AREA070C.bif"
        )));

        let bif = BifcParser::import(&mut data.reader().unwrap()).unwrap();
        assert_eq!(bif.r#type, Type::Bifc);
    }

}