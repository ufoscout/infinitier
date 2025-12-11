mod bif_reader;
mod bifc_reader;
mod biff_reader;

use std::io::Read;

use crate::{
    datasource::{Importer, Reader},
    resource::{
        bif::{bif_reader::BifParser, bifc_reader::BifcParser, biff_reader::BiffParser},
        key::ResourceType,
    },
};

/// A BIF file importer
pub struct BifImporter {}

impl Importer for BifImporter {
    type T = Bif;

    fn import(source: &crate::datasource::DataSource) -> std::io::Result<Self::T> {
        let reader = &mut source.reader()?;
        let position = reader.position()?;

        match detect_biff_type(reader)? {
            Type::Biff => {
                reader.set_position(position)?;
                BiffParser::import(reader)
            }
            Type::Bif => {
                reader.set_position(position)?;
                BifParser::import(reader)
            }
            Type::Bifc => {
                reader.set_position(position)?;
                BifcParser::import(reader)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    Biff, // BIFF V1
    Bif,  // BIFC V1   (compressed)
    Bifc, // BIFC V1.0 (compressed)
}

pub const BIFFV1_SIGNATURE: &str = "BIFFV1  ";
pub const BIF_V1_0_SIGNATURE: &str = "BIF V1.0";
pub const BIFCV1_0_SIGNATURE: &str = "BIFCV1.0";

impl Type {
    pub fn signature(&self) -> &'static str {
        match self {
            Type::Biff => BIFFV1_SIGNATURE,
            Type::Bif => BIF_V1_0_SIGNATURE,
            Type::Bifc => BIFCV1_0_SIGNATURE,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Bif {
    pub r#type: Type,
    pub files: Vec<BifEmbeddedFile>,
    pub tilesets: Vec<BifEmbeddedTileset>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BifEmbeddedFile {
    pub locator: u32,
    pub size: u32,
    pub offset: u64,
    pub r#type: ResourceType,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BifEmbeddedTileset {
    pub locator: u32,
    pub size: u32,
    pub count: u32,
    pub offset: u64,
    pub r#type: ResourceType,
}

/// Detects the type of a BIFF file
fn detect_biff_type<R: Read>(reader: &mut Reader<R>) -> std::io::Result<Type> {
    let value = reader.read_string(8)?;

    match value.as_str() {
        BIFFV1_SIGNATURE => Ok(Type::Biff),
        BIF_V1_0_SIGNATURE => Ok(Type::Bif),
        BIFCV1_0_SIGNATURE => Ok(Type::Bifc),
        val => Err(std::io::Error::other(format!(
            "Unsupported BIFF file: {}",
            val
        ))),
    }
}

fn parse_bif_embedded_file<R: Read>(reader: &mut Reader<R>) -> std::io::Result<BifEmbeddedFile> {
    let locator = reader.read_u32()? & 0xfffff;
    let offset = reader.read_u32()? as u64;
    let size = reader.read_u32()?;
    let r#type = reader.read_u16()?;
    reader.read_u16()?; // unknown data

    Ok(BifEmbeddedFile {
        locator,
        offset,
        size,
        r#type: ResourceType::from(r#type),
    })
}

fn parse_bif_embedded_tileset<R: Read>(
    reader: &mut Reader<R>,
) -> std::io::Result<BifEmbeddedTileset> {
    let locator = reader.read_u32()? & 0xfffff;
    let offset = reader.read_u32()? as u64;
    let count = reader.read_u32()?;
    let size = reader.read_u32()?;
    let r#type = reader.read_u16()?;
    reader.read_u16()?; // unknown data

    Ok(BifEmbeddedTileset {
        locator,
        offset,
        count,
        size,
        r#type: ResourceType::from(r#type),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};

    use super::*;

    #[test]
    fn test_detect_bif_type() {
        let data = DataSource::new(Path::new(&format!(
            "{RESOURCES_DIR}iwd/CD2/Data/AR3603.cbf"
        )));

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Bif
        );

        let bif = BifParser::import(&mut data.reader().unwrap()).unwrap();
        assert_eq!(bif.r#type, Type::Bif);
    }

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
    }

    #[test]
    fn test_detect_biff_type() {
        let data = DataSource::new(Path::new(&format!("{RESOURCES_DIR}pst/CS_0511.bif")));

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Biff
        );

        let bif = BiffParser::import(&mut data.reader().unwrap()).unwrap();
        assert_eq!(bif.r#type, Type::Biff);
    }
}
