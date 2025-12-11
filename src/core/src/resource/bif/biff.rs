use std::io::{Read, Seek};

use crate::{
    datasource::Reader,
    resource::bif::{Bif, Type, parse_bif_embedded_file, parse_bif_embedded_tileset},
};

/// A BIFF V1 file importer
pub struct BiffParser;

impl BiffParser {

    /// Imports a BIFF V1 file
    pub fn import<R: Read + Seek>(reader: &mut Reader<R>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq("BIFFV1  ") {
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let files_number = reader.read_u32()? as usize;
        let tilesets_number = reader.read_u32()? as usize;
        let files_offset = reader.read_u32()? as u64;

        reader.set_position(files_offset)?;

        let mut bif = Bif {
            r#type: Type::Biff,
            files: Vec::with_capacity(files_number),
            tilesets: Vec::with_capacity(tilesets_number),
        };

        // reading file entries
        for _ in 0..files_number {
            bif.files.push(parse_bif_embedded_file(reader)?)
        }

        // reading tileset entries
        for _ in 0..tilesets_number {
            bif.tilesets.push(parse_bif_embedded_tileset(reader)?)
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
    fn test_detect_biff_type() {
        let data = DataSource::new(Path::new(&format!("{RESOURCES_DIR}pst/CS_0511.bif")));

        let bif = BiffParser::import(&mut data.reader().unwrap()).unwrap();

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Biff
        );

        assert_eq!(bif.r#type, Type::Biff);
        assert_eq!(bif.files.len(), 4);
        assert_eq!(bif.tilesets.len(), 0);

        assert_eq!(
            bif.files[1],
            BifEmbeddedFile {
                locator: 1,
                size: 4050,
                offset: 7952,
                r#type: ResourceType::Bcs
            }
        );
        assert_eq!(
            bif.files[3],
            BifEmbeddedFile {
                locator: 3,
                size: 285,
                offset: 17222,
                r#type: ResourceType::Bcs
            }
        );
    }

    #[test]
    fn test_import_biff() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}bg2_ee/data/area500c.bif"
        )));

        let bif = BiffParser::import(&mut data.reader().unwrap()).unwrap();

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Biff
        );

        assert_eq!(bif.r#type, Type::Biff);
        assert_eq!(bif.files.len(), 5);
        assert_eq!(bif.tilesets.len(), 1);

        assert_eq!(
            bif.files[0],
            BifEmbeddedFile {
                locator: 0,
                size: 315816,
                offset: 24,
                r#type: ResourceType::Mos
            }
        );
        assert_eq!(
            bif.tilesets[0],
            BifEmbeddedTileset {
                locator: 16384,
                size: 12,
                offset: 461932,
                count: 2507,
                r#type: ResourceType::Tis
            }
        );
    }
}
