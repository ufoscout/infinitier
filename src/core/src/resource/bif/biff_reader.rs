use std::io::{Read, Seek};

use crate::{
    datasource::Reader,
    resource::bif::{
        BIFFV1_SIGNATURE, Bif, Type, parse_bif_embedded_file, parse_bif_embedded_tileset,
    },
};

/// A BIFF V1 file importer
pub struct BiffParser;

impl BiffParser {
    /// Imports a BIFF V1 file
    pub fn import<R: Read + Seek>(reader: &mut Reader<R>) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq(BIFFV1_SIGNATURE) {
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
            resources: Vec::with_capacity(files_number + tilesets_number),
        };

        // reading file entries
        for _ in 0..files_number {
            bif.resources.push(parse_bif_embedded_file(reader)?)
        }

        // reading tileset entries
        for _ in 0..tilesets_number {
            bif.resources.push(parse_bif_embedded_tileset(reader)?)
        }

        Ok(bif)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        datasource::DataSource,
        resource::{
            bif::{BifEmbeddedResource, detect_biff_type},
            key::ResourceType,
        },
        test_utils::RESOURCES_DIR,
    };

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
        assert_eq!(bif.resources.len(), 4);

        assert_eq!(
            bif.resources[1],
            BifEmbeddedResource::File {
                locator: 1,
                size: 4050,
                offset: 7952,
                r#type: ResourceType::Bcs
            }
        );
        assert_eq!(
            bif.resources[3],
            BifEmbeddedResource::File {
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

        println!("{:#?}", bif);

        assert_eq!(bif.r#type, Type::Biff);
        assert_eq!(bif.resources.len(), 6);

        assert_eq!(
            bif.resources[0],
            BifEmbeddedResource::File {
                locator: 0,
                size: 315816,
                offset: 24,
                r#type: ResourceType::Mos
            }
        );
        assert_eq!(
            bif.resources[5],
            BifEmbeddedResource::Tileset {
                locator: 16384,
                size: 12,
                offset: 461932,
                count: 2507,
                r#type: ResourceType::Tis
            }
        );
    }
}
