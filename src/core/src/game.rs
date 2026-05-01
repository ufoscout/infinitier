use std::{
    io,
    path::{Path, PathBuf},
};

use infinitier_bif_importer::{BifEmbeddedResource, BifImporter};
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};
use infinitier_key_importer::{Key, KeyImporter, ResourceType};
use log::{debug, warn};

pub type ResourceId = usize;

/// The Data of a game.
#[derive(Debug, PartialEq, Eq)]
pub struct GameData {
    pub resources: Vec<GameResource>,
}

/// A game resource
#[derive(Debug, PartialEq, Eq)]
pub struct GameResource {
    /// Resource name without extension.
    pub name: String,
    /// Resource type.
    pub r#type: ResourceType,
    /// Filename: `name.extension`
    pub filename: String,
    /// File size
    pub file_size: Option<u64>,
    /// Data source
    pub datasource: Option<DataSource>,
    /// Has an override
    pub has_override: bool,
}

/// A game data builder
pub struct GameDataBuilder {
    /// File system
    fs: CaseInsensitiveFS,
    /// Name of the key file
    key_file: String,
    /// Resource overrides folders
    overrides: Vec<String>,
    /// Resource fallback folders
    fallbacks: Vec<String>,
}

impl GameDataBuilder {
    /// Create a new game data builder
    pub fn new<P: AsRef<Path>>(game_root: P) -> io::Result<GameDataBuilder> {
        Ok(GameDataBuilder {
            fs: CaseInsensitiveFS::new(game_root)?,
            overrides: vec!["override".to_string()],
            key_file: "chitin.key".to_string(),
            fallbacks: vec![
                "data".to_string(),
                "cache".to_string(),
                "cd1".to_string(),
                "cd2".to_string(),
                "cd3".to_string(),
                "cd4".to_string(),
                "cd5".to_string(),
                "cd6".to_string(),
                "cd7".to_string(),
            ],
        })
    }

    /// Set the key file name
    /// Default: "chitin.key"
    pub fn with_key_file(mut self, key_file: String) -> GameDataBuilder {
        self.key_file = key_file;
        self
    }

    /// Set the fallback folders.
    /// Default: ["data", "cache", "cd1", "cd2", "cd3", "cd4", "cd5", "cd6", "cd7"]
    pub fn with_fallbacks(mut self, fallbacks: Vec<String>) -> GameDataBuilder {
        self.fallbacks = fallbacks;
        self
    }

    /// Set the resource override folders
    /// Default: ["override"]
    pub fn with_overrides(mut self, overrides: Vec<String>) -> GameDataBuilder {
        self.overrides = overrides;
        self
    }

    /// Build the game data
    pub fn build(&self) -> io::Result<GameData> {
        let mut game_data = GameData { resources: vec![] };

        let key_path = self
            .fs
            .get_path(&CaseInsensitivePath::new(&self.key_file))?;
        let key = KeyImporter::import(&DataSource::new(key_path.as_path()))?;

        for resource in key.resource_entries {
            let name = resource.resource_name;
            let r#type = resource.r#type;
            let filename = match r#type.get_extension() {
                Some(ext) => &format!("{}.{}", name, ext),
                None => &name,
            };
            let cs_path = CaseInsensitivePath::new(filename);
            let filename = cs_path.base_name().to_string();

            if let Some(r#override) = self.search_override(&cs_path) {
                // The resource has an override so we use the override instead of the bif file
                debug!("Resource {} has an override", filename);
                let file_size = Some(r#override.metadata()?.len());
                let datasource = Some(DataSource::new(r#override.as_path()));
                game_data.resources.push(GameResource {
                    name,
                    r#type,
                    filename,
                    file_size,
                    datasource,
                    has_override: true,
                });
            } else {
                if let Some(bif_entry) = key
                    .bif_entries
                    .get(resource.bif_entries_index as usize)
                    .and_then(|bif| {
                        self.fs
                            .get_path_opt(&CaseInsensitivePath::new(&bif.file_name))
                    })
                {
                    // ToDo: read bif files only once
                    let to_do = 0;
                    let bif = BifImporter::import(&DataSource::new(bif_entry.as_path()))?;

                    if let Some(bif_resource) =
                        bif.resources.get(resource.index_into_bif_file as usize)
                    {
                        debug!(
                            "Resource {} found in bif {}",
                            filename,
                            bif_entry.as_path().display()
                        );

                        let (datasource, file_size) = match bif_resource {
                            BifEmbeddedResource::File {
                                locator,
                                size,
                                offset,
                                r#type,
                            } => (
                                DataSource::new_with_offset(
                                    bif_entry.as_path(),
                                    *offset,
                                    Some(*size as u64),
                                ),
                                *size as u64,
                            ),
                            BifEmbeddedResource::Tileset {
                                locator,
                                size,
                                count,
                                offset,
                                r#type,
                            } => (
                                DataSource::new_with_offset(
                                    bif_entry.as_path(),
                                    *offset,
                                    Some(*size as u64),
                                ),
                                *size as u64,
                            ),
                        };

                        game_data.resources.push(GameResource {
                            name,
                            r#type,
                            filename,
                            file_size: Some(file_size),
                            datasource: Some(datasource),
                            has_override: false,
                        });
                    } else {
                        warn!(
                            "Resource {} not found in bif {}",
                            filename,
                            bif_entry.as_path().display()
                        );
                        game_data.resources.push(GameResource {
                            name,
                            r#type,
                            filename,
                            file_size: None,
                            datasource: None,
                            has_override: false,
                        });
                    }
                } else {
                    warn!("Resource {} not found", filename);
                    game_data.resources.push(GameResource {
                        name,
                        r#type,
                        filename,
                        file_size: None,
                        datasource: None,
                        has_override: false,
                    });
                }
            }
        }

        Ok(game_data)
    }

    /// Search for a resource override
    fn search_override(&self, cs_path: &CaseInsensitivePath) -> Option<PathBuf> {
        for r#override in self.overrides.iter() {
            let search_name = format!("{}/{}", r#override, cs_path.base_name());
            if let Some(path) = self
                .fs
                .search_path_opt(&CaseInsensitivePath::new(&search_name))
            {
                return Some(path);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use infinitier_test_utils::{constants::BG2_RESOURCES_DIR, get_assets_path};

    use super::*;

    #[test]
    fn test_game_data_builder() {
        let bg_root = get_assets_path().join(BG2_RESOURCES_DIR);
        let game_data = GameDataBuilder::new(bg_root).unwrap()
            .build()
            .unwrap();
        assert_eq!(game_data.resources.len(), 41793);


    }
}
