use std::{collections::VecDeque, io::{BufRead, Read}};
use crate::{datasource::Reader, resource::bif::{Bif, Type, parse_bif_embedded_file, parse_bif_embedded_tileset}};

/// A BIFC V1.0 file importer
pub struct BifcParser;

impl BifcParser {

    /// Imports a BIFC V1.0 file
    pub fn import<'a: 'b, 'b, R: BufRead>(reader: &'b mut Reader<R>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq("BIFCV1.0") {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        };

        let uncompressed_size = reader.read_u32()?;

        let bif = {

            let mut zip = Reader{
                charset: reader.charset,
                data: BifcCompressedReader{
                    reader,
                    buffer: VecDeque::new()
                }, 
            };
        let signature = zip.read_string(8)?;

        if !signature.eq("BIFFV1  ") {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let files_number = zip.read_u32()? as usize;
        let tilesets_number = zip.read_u32()? as usize;
        let files_offset = zip.read_u32()? as u64;

        let current_offset = 20;
        if files_offset < current_offset {
            return Err(std::io::Error::other(format!(
                "Invalid decompressed BIFF header offset: {}",
                files_offset
            )));
        }

        let remaining_bytes = files_offset - current_offset;

        zip.skip(remaining_bytes)?;

        let mut bif = Bif {
            r#type: Type::Bifc,
            files: Vec::with_capacity(files_number),
            tilesets: Vec::with_capacity(tilesets_number),
        };

        // reading file entries
        for _ in 0..files_number {
            bif.files.push(parse_bif_embedded_file(&mut zip)?);
        }

        // reading tileset entries
        for _ in 0..tilesets_number {
            bif.tilesets.push(parse_bif_embedded_tileset(&mut zip)?);
        }

            bif
        };

        Ok(bif)
    }
}


struct BifcCompressedReader<'a, R: BufRead>{
    reader: &'a mut Reader<R>,
    buffer: VecDeque<u8>
}

impl <'a, R: BufRead> Read for BifcCompressedReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();

        if self.buffer.len() < len {
            self.fill_buffer()?;
        }

        let len = std::cmp::min(len, self.buffer.len());
        self.buffer.read(buf)?;

        Ok(len)
    }
}

impl <'a, R: BufRead> BifcCompressedReader<'a, R> {
    fn fill_buffer(&mut self) -> std::io::Result<usize> {

        println!("Filling buffer");

        let uncompressed_size = self.reader.read_u32()? as u64;
        let compressed_size = self.reader.read_u32()? as u64;

        let mut take = self.reader.take(compressed_size);
        let mut reader = take.as_zip_reader();

        // Inefficient but works for now
        let data = reader.take_to_vec(uncompressed_size)?;
        
        self.buffer = VecDeque::from(data);

        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::{datasource::DataSource, resource::bif::detect_biff_type, test_utils::RESOURCES_DIR};
    use super::*;


        #[test]
    fn test_detect_bifc_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}bg2/data/Data/AREA070C.bif"
        )));

                assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Bifc
        );
        
        let bif = BifcParser::import(&mut data.reader().unwrap()).unwrap();
        assert_eq!(bif.r#type, Type::Bifc);

        println!("{:#?}", bif);

    }

}