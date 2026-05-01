use std::{io::{BufRead, Cursor}, sync::Arc};

use infinitier_datasource::Reader;
use log::{debug, error};

use crate::{BIF_V1_0_SIGNATURE, Bif, Type, biff_reader::BiffParser};

/// A BIFC V1 file importer
pub struct BifParser;

impl BifParser {
    /// Imports a BIF V1.0 (compressed) file.
    /// Decompresses the payload into memory so resource offsets refer to the
    /// decompressed buffer.
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;
        if !signature.eq(BIF_V1_0_SIGNATURE) {
            error!("Not a BIF V1.0 file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let name_length = reader.read_u32()? as u64;
        let _name = reader.read_string(name_length)?;
        let _uncompressed_data_length = reader.read_u32()? as u64;
        let _compressed_data_length = reader.read_u32()? as u64;

        // Decompress the entire payload into memory.
        let decompressed = {
            let mut zip = reader.as_zip_reader();
            let decoded = zip.decode_all()?;
            decoded.data.into_inner()
        };

        // Parse the embedded BIFF V1 from the decompressed bytes.
        let resources = {
            let cursor = Cursor::new(decompressed.as_slice());
            let mut inner = Reader { data: cursor, charset: reader.charset };
            let mut bif = BiffParser::import(&mut inner)?;
            bif.resources
        };

        let data = Arc::new(decompressed);
        debug!("Loaded BIF V1.0: {} resources", resources.len());
        Ok(Bif { r#type: Type::Bif, resources, data: Some(data) })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_key_importer::ResourceType;
    use infinitier_test_utils::get_assets_path;

    use crate::{BifEmbeddedResource, detect_biff_type};

    use super::*;

    #[test]
    fn test_detect_bif_type() {
        let data = DataSource::new(get_assets_path().join("iwd/CD2/Data/AR3603.cbf"));

        let mut reader = data.reader().unwrap();
        let bif = BifParser::import(&mut reader).unwrap();

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Bif
        );

        assert_eq!(bif.r#type, Type::Bif);
        assert_eq!(bif.resources.len(), 6);

        assert_eq!(
            bif.resources[0],
            BifEmbeddedResource::File {
                locator: 0,
                size: 3850,
                offset: 120,
                r#type: ResourceType::Wed
            }
        );
        assert_eq!(
            bif.resources[2],
            BifEmbeddedResource::File {
                locator: 2,
                size: 7480,
                offset: 7288,
                r#type: ResourceType::Bmp
            }
        );
        assert_eq!(
            bif.resources[5],
            BifEmbeddedResource::Tileset {
                locator: 16384,
                size: 5120,
                offset: 43480,
                count: 300,
                r#type: ResourceType::Tis
            }
        );
    }
}
