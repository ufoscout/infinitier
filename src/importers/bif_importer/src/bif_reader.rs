use std::sync::Arc;

use infinitier_datasource::{Data, DataSource, TempFileGenerator};
use log::{debug, error};

use crate::{BIF_V1_0_SIGNATURE, Bif, Type, biff_reader::BiffParser};

/// A BIFC V1 file importer
pub struct BifParser;

impl BifParser {
    /// Imports a BIF V1.0 (compressed) file.
    /// Decompresses the payload into a temporary file so resource offsets refer to the
    /// decompressed buffer.
    pub fn import(source: &DataSource, name: &str) -> std::io::Result<Bif> {
        let reader = &mut source.reader()?;
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

        // Decompress the BIF lazily into a temporary file.
        // Every read will be performed from this temporary file.
        let lazy_datasource = {
            let temp_dir = tempfile::Builder::new().prefix("infinitier_").tempdir()?;
            let source_clone = source.clone();
            DataSource::new(Data::Generator(Arc::new(TempFileGenerator::new(Box::new(
                move || {
                    let path = temp_dir.path().join("bif.bif");
                    let mut temp_file = std::fs::File::create(&path)?;

                    let reader = &mut source_clone.reader()?;

                    // Throw away the first bytes
                    {
                        let _signature = reader.read_string(8)?;
                        let name_length = reader.read_u32()? as u64;
                        let _name = reader.read_string(name_length)?;
                        let _uncompressed_data_length = reader.read_u32()? as u64;
                        let _compressed_data_length = reader.read_u32()? as u64;
                    }

                    let mut zip = reader.as_zip_reader();
                    zip.copy(&mut temp_file)?;
                    Ok(path)
                },
            )))))
        };

        let resources = BiffParser::parse_resources(&mut lazy_datasource.reader()?)?;

        debug!("Loaded {name} [BIF V1.0]: {} resources", resources.len());
        Ok(Bif {
            name: name.to_string(),
            r#type: Type::Bif,
            resources,
            datasource: lazy_datasource,
        })
    }
}

#[cfg(test)]
mod tests {
    use infinitier_common::ResourceType;
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::get_assets_path;

    use crate::{BifEmbeddedResource, detect_biff_type};

    use super::*;

    #[test]
    fn test_detect_bif_type() {
        let data = DataSource::new(get_assets_path().join("KEY/iwd/CD2/Data/AR3603.cbf"));

        let bif = BifParser::import(&data, "bif_name").unwrap();

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Bif
        );

        assert_eq!(bif.name, "bif_name");
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
