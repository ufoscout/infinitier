use infinitier_datasource::{DataSource, Reader};
use log::{debug, error};

use crate::{BIFCV1_0_SIGNATURE, Bif, Type, biff_reader::BiffParser};
use std::{
    collections::VecDeque,
    io::{BufRead, Cursor, Read, Seek},
};

/// A BIFC V1.0 file importer
pub struct BifcParser;

impl BifcParser {
    /// Imports a BIFC V1.0 file.
    /// Decompresses the entire archive into memory so that resource offsets
    /// can be used to slice the decompressed buffer directly.
    pub fn import<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;
        if !signature.eq(BIFCV1_0_SIGNATURE) {
            error!("Not a BIFC V1.0 file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }
        let total_uncompressed_size = reader.read_u32()? as u64;

        // Decompress the entire BIFC payload into memory.
        let decompressed = {
            let mut bytes = Vec::with_capacity(total_uncompressed_size as usize);
            let mut cr = BifcCompressedReader::new(reader, total_uncompressed_size);
            cr.read_to_end(&mut bytes)?;
            bytes
        };

        // Parse the embedded BIFF V1 resource table from the decompressed bytes.
        let resources = {
            let cursor = Cursor::new(decompressed.as_slice());
            let mut inner = Reader { data: cursor, charset: reader.charset };
            BiffParser::parse_resources(&mut inner)?
        };

        let datasource = DataSource::new(decompressed);
        debug!("Loaded BIFC V1.0: {} resources", resources.len());
        Ok(Bif { r#type: Type::Bifc, resources, datasource })
    }
}

struct BifcCompressedReader<'a, R: BufRead> {
    reader: &'a mut Reader<R>,
    buffer: VecDeque<u8>,
    offset: u64,
    total_size: u64,
}

impl<'a, R: BufRead + Seek> Read for BifcCompressedReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.offset >= self.total_size {
            return Ok(0);
        }

        let len = buf.len();
        if self.buffer.is_empty() {
            self.fill_buffer()?;
        }

        let len = std::cmp::min(len, self.buffer.len());
        self.buffer.read(buf)?;
        self.offset += len as u64;

        Ok(len)
    }
}

impl<'a, R: BufRead + Seek> BifcCompressedReader<'a, R> {
    fn new(reader: &'a mut Reader<R>, total_size: u64) -> Self {
        BifcCompressedReader {
            reader,
            buffer: VecDeque::new(),
            offset: 0,
            total_size,
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
    use infinitier_datasource::DataSource;
    use infinitier_key_importer::ResourceType;
    use infinitier_test_utils::get_assets_path;

    use super::*;
    use crate::{BifEmbeddedResource, detect_biff_type};

    #[test]
    fn test_detect_bifc_type() {
        let data = DataSource::new(get_assets_path().join("bg2/data/Data/AREA070C.bif"));

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
