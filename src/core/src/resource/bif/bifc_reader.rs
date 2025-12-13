use crate::{
    datasource::Reader,
    resource::bif::{
        BIFCV1_0_SIGNATURE, BIFFV1_SIGNATURE, Bif, Type, parse_bif_embedded_file,
        parse_bif_embedded_tileset,
    },
};
use std::{
    collections::VecDeque,
    io::{BufRead, Read, Seek},
};

/// A BIFC V1.0 file importer
pub struct BifcParser;

impl BifcParser {
    /// Imports a BIFC V1.0 file
    pub fn import<'a: 'b, 'b, R: BufRead + Seek>(
        reader: &'b mut Reader<R>,
    ) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq(BIFCV1_0_SIGNATURE) {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        };

        let _uncompressed_size = reader.read_u32()?;

        let bif = {
            let mut zip = Reader {
                charset: reader.charset,
                data: BifcCompressedReader::new(reader),
            };
            let signature = zip.read_string(8)?;

            if !signature.eq(BIFFV1_SIGNATURE) {
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
                resources: Vec::with_capacity(files_number + tilesets_number),
            };

            // reading file entries
            for _ in 0..files_number {
                bif.resources.push(parse_bif_embedded_file(&mut zip)?);
            }

            // reading tileset entries
            for _ in 0..tilesets_number {
                bif.resources.push(parse_bif_embedded_tileset(&mut zip)?);
            }

            bif
        };

        Ok(bif)
    }
}

struct BifcCompressedReader<'a, R: BufRead> {
    reader: &'a mut Reader<R>,
    buffer: VecDeque<u8>,
    offset: u64,
}

impl<'a, R: BufRead + Seek> Read for BifcCompressedReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();

        if self.buffer.len() < len {
            self.fill_buffer()?;
        }

        let len = std::cmp::min(len, self.buffer.len());
        self.buffer.read(buf)?;

        self.offset += len as u64;

        Ok(len)
    }
}

impl<'a, R: BufRead + Seek> BifcCompressedReader<'a, R> {
    fn new(reader: &'a mut Reader<R>) -> Self {
        BifcCompressedReader {
            reader,
            buffer: VecDeque::new(),
            offset: 0,
        }
    }

    fn fill_buffer(&mut self) -> std::io::Result<usize> {
        // uncompressed_size can be used to skip bytes without decompression based on the offset
        let _uncompressed_size = self.reader.read_u32()? as u64;
        let compressed_size = self.reader.read_u32()? as u64;

        let mut take = self.reader.take(compressed_size);
        let mut reader = take.as_zip_reader();

        {
            // A reasonably sized stack buffer (adjust if needed)
            let mut buf = [0_u8; 8192];

            loop {
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break; // EOF
                }
                self.buffer.extend(&buf[..n]);
            }
        }

        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        datasource::DataSource,
        resource::{
            bif::{BifEmbeddedResource, detect_biff_type},
            key::ResourceType,
        },
        test_utils::RESOURCES_DIR,
    };
    use std::path::Path;

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

        assert_eq!(bif.resources.len(), 6);

        assert_eq!(
            bif.resources[1],
            BifEmbeddedResource::File {
                locator: 1,
                size: 3574,
                offset: 4204,
                r#type: ResourceType::Bmp
            }
        );
        assert_eq!(
            bif.resources[4],
            BifEmbeddedResource::File {
                locator: 4,
                size: 98002,
                offset: 19342,
                r#type: ResourceType::Wav
            }
        );

        assert_eq!(
            bif.resources[5],
            BifEmbeddedResource::Tileset {
                locator: 16384,
                size: 5120,
                count: 324,
                offset: 117344,
                r#type: ResourceType::Tis
            }
        );
    }
}
