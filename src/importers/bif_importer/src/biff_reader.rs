use std::io::{Read, Seek};

use infinitier_datasource::{DataSource, Reader};
use log::{debug, error};

use crate::{BIFFV1_SIGNATURE, Bif, BifEmbeddedResource, Type, parse_bif_embedded_file, parse_bif_embedded_tileset};

/// A BIFF V1 file importer
pub struct BiffParser;

impl BiffParser {
    /// Imports a BIFF V1 file from a DataSource.
    pub fn import(source: &DataSource) -> std::io::Result<Bif> {
        let reader = &mut source.reader()?;
        let resources = Self::parse_resources(reader)?;
        debug!("Loaded BIFF V1: {} resources", resources.len());
        Ok(Bif { r#type: Type::Biff, resources, datasource: source.clone() })
    }

    /// Parses BIFF V1 resource entries from an existing reader.
    /// Used internally by compressed BIF parsers that decompress into a buffer first.
    pub(crate) fn parse_resources<R: Read + Seek>(
        reader: &mut Reader<R>,
    ) -> std::io::Result<Vec<BifEmbeddedResource>> {
        let signature = reader.read_string(8)?;

        if !signature.eq(BIFFV1_SIGNATURE) {
            error!("Not a BIFF V1 file: {:?}", signature);
            return Err(std::io::Error::other(format!(
                "Wrong file type: {}",
                signature
            )));
        }

        let files_number = reader.read_u32()? as usize;
        let tilesets_number = reader.read_u32()? as usize;
        let files_offset = reader.read_u32()? as u64;

        reader.set_position(files_offset)?;

        let mut resources = Vec::with_capacity(files_number + tilesets_number);
        for _ in 0..files_number {
            resources.push(parse_bif_embedded_file(reader)?);
        }
        for _ in 0..tilesets_number {
            resources.push(parse_bif_embedded_tileset(reader)?);
        }
        Ok(resources)
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
    fn test_detect_biff_type() {
        let data = DataSource::new(get_assets_path().join("pst/CS_0511.bif"));

        let bif = BiffParser::import(&data).unwrap();

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
        let data = DataSource::new(get_assets_path().join("bg2_ee/data/area500c.bif"));

        let bif = BiffParser::import(&data).unwrap();

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
