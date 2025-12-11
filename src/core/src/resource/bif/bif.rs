use std::io::BufRead;

use crate::{
    datasource::Reader,
    resource::bif::{BIF_V1_0_SIGNATURE, BIFFV1_SIGNATURE, Bif, Type, parse_bif_embedded_file, parse_bif_embedded_tileset},
};

/// A BIFC V1 file importer
pub struct BifParser;

impl BifParser {

    /// Imports a BIFC V1 file
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq(BIF_V1_0_SIGNATURE) {
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

        Ok(bif)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{datasource::DataSource, resource::{bif::{BifEmbeddedFile, BifEmbeddedTileset, detect_biff_type}, key::ResourceType}, test_utils::RESOURCES_DIR};

    use super::*;

    #[test]
    fn test_detect_bif_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}iwd/CD2/Data/AR3603.cbf"
        )));

        let mut reader = data.reader().unwrap();
        let bif = BifParser::import(&mut reader).unwrap();

                assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Bif
        );

        assert_eq!(bif.r#type, Type::Bif);
        assert_eq!(bif.files.len(), 5);
        assert_eq!(bif.tilesets.len(), 1);

        assert_eq!(
            bif.files[0],
            BifEmbeddedFile {
                locator: 0,
                size: 3850,
                offset: 120,
                r#type: ResourceType::Wed
            }
        );
        assert_eq!(
            bif.files[2],
            BifEmbeddedFile {
                locator: 2,
                size: 7480,
                offset: 7288,
                r#type: ResourceType::Bmp
            }
        );
        assert_eq!(
            bif.tilesets[0],
            BifEmbeddedTileset {
                locator: 16384,
                size: 5120,
                offset: 43480,
                count: 300,
                r#type: ResourceType::Tis
            }
        );
    }

}
