use crate::datasource::{DataSource, Importer, Reader};

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    Biff, // BIFF V1
    Bif,  // BIFC V1   (compressed)
    Bifc, // BIFC V1.0 (compressed)
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
    pub r#type: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BifEmbeddedTileset {
    pub locator: u32,
    pub size: u32,
    pub count: u32,
    pub offset: u64,
    pub r#type: u16,
}

/// Detects the type of a BIFF file
fn detect_biff_type(reader: &mut Reader) -> std::io::Result<Type> {
    let value = reader.read_string(8)?;

    match value.as_str() {
        "BIFFV1  " => Ok(Type::Biff),
        "BIF V1.0" => Ok(Type::Bif),
        "BIFCV1.0" => Ok(Type::Bifc),
        val => Err(std::io::Error::other(format!(
            "Unsupported BIFF file: {}",
            val
        ))),
    }
}

pub struct BiffImporter;

impl BiffImporter {
    pub fn import(reader: &mut Reader) -> std::io::Result<Bif> {
        let signature = reader.read_string(8)?;

        if !signature.eq("BIFFV1  ") {
            return Err(std::io::Error::other(format!("Wrong file type: {}", signature)));
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
        for i in 0..files_number {
            let locator = reader.read_u32()? & 0xfffff;
            let offset = reader.read_u32()? as u64;
            let size = reader.read_u32()?;
            let r#type = reader.read_u16()?;
            reader.read_u16()?; // unknown data

            bif.files.push(BifEmbeddedFile {
                locator,
                offset,
                size,
                r#type,
            })
            // addEntry(new Entry(locator, offset, size, type));
        }

        // reading tileset entries
        for i in 0..tilesets_number {
                        let locator = reader.read_u32()? & 0xfffff;
                let offset = reader.read_u32()? as u64;
                let count = reader.read_u32()?;
                let size = reader.read_u32()?;
                let r#type = reader.read_u16()?;
                reader.read_u16()?; // unknown data

                bif.tilesets.push(BifEmbeddedTileset {
                    locator,
                    offset,
                    count,
                    size,
                    r#type,
                })
                // addEntry(new Entry(locator, offset, count, size, type));
        }

        Ok(bif)
    }
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
    }

    #[test]
    fn test_detect_biff_type() {
        let data = DataSource::new(Path::new(&format!("{RESOURCES_DIR}pst/CS_0511.bif")));

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Biff
        );

        let bif = BiffImporter::import(&mut data.reader().unwrap()).unwrap();

        assert_eq!(bif.files.len(), 4);
        assert_eq!(bif.tilesets.len(), 0);

        assert_eq!(bif.files[1], BifEmbeddedFile {
            locator: 1,
            size: 4050,
            offset: 7952,
            r#type: 1007
        });
        assert_eq!(bif.files[3], BifEmbeddedFile {
            locator: 3,
            size: 285,
            offset: 17222,
            r#type: 1007
        });
    }

    #[test]
    fn test_import_biff() {
        let data = DataSource::new(Path::new(&format!("{RESOURCES_DIR}bg2_ee/data/area500c.bif")));

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Biff
        );

        let bif = BiffImporter::import(&mut data.reader().unwrap()).unwrap();

        assert_eq!(bif.files.len(), 5);
        assert_eq!(bif.tilesets.len(), 1);

        assert_eq!(bif.files[0], BifEmbeddedFile {
            locator: 0,
            size: 315816,
            offset: 24,
            r#type: 1004
        });
        assert_eq!(bif.tilesets[0], BifEmbeddedTileset {
            locator: 16384,
            size: 12,
            offset: 461932,
            count: 2507,
            r#type: 1003
        });
    }
}
