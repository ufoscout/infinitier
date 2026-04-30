use std::io::BufRead;

use infinitier_datasource::Reader;
use log::{debug, error};

use crate::{
    BIF_V1_0_SIGNATURE, BIFFV1_SIGNATURE, Bif, Type, parse_bif_embedded_file,
    parse_bif_embedded_tileset,
};

/// A BIFC V1 file importer
pub struct BifParser;

impl BifParser {
    /// Imports a BIFC V1 file
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

        let _uncompressed_data_lenght = reader.read_u32()? as u64;
        let _compressed_data_lenght = reader.read_u32()? as u64;

        let mut zip = reader.as_zip_reader();

        let signature = zip.read_string(8)?;

        if !signature.eq(BIFFV1_SIGNATURE) {
            error!("BIF V1.0 inner decompressed signature not BIFF V1: {:?}", signature);
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
            r#type: Type::Bif,
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

        debug!("Loaded BIF V1.0: {} resources", bif.resources.len());
        Ok(bif)
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
