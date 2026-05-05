use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use infinitier_bif_importer::{BifEmbeddedResource, BifImporter};
use infinitier_common::{Game, ResourceType};
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};
use infinitier_key_importer::KeyImporter;
use log::{debug, warn};

pub type ResourceId = usize;

/// The Data of a game.
#[derive(Debug)]
pub struct GameData {
    /// Game Type
    game_type: Game,
    /// All resources
    resources: Vec<GameResource>,
    /// A map from filename to resource id
    filename_index: HashMap<String, ResourceId>,
    /// A map from (name, type) to resource id
    name_type_index: HashMap<(String, ResourceType), ResourceId>,
}

impl GameData {
    /// Return the number of resources
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Return true if there are no resources
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Return all resources
    pub fn resources(&self) -> &[GameResource] {
        &self.resources
    }

    /// Get a resource by id
    pub fn get_by_id(&self, id: ResourceId) -> Option<&GameResource> {
        self.resources.get(id)
    }

    /// Get a resource by filename
    pub fn get_by_filename(&self, filename: &str) -> Option<&GameResource> {
        self.filename_index
            .get(filename)
            .and_then(|&id| self.resources.get(id))
    }

    /// Get a resource by name and type
    pub fn get_by_name_and_type(&self, name: &str, r#type: ResourceType) -> Option<&GameResource> {
        self.name_type_index
            .get(&(name.to_string(), r#type))
            .and_then(|&id| self.resources.get(id))
    }

    /// Creates a GameData from a list of resources
    pub fn new(resources: Vec<GameResource>, game_type: Game) -> Self {
        let mut game_data = GameData {
            game_type,
            resources: Vec::new(),
            filename_index: HashMap::new(),
            name_type_index: HashMap::new(),
        };
        for resource in resources {
            game_data.add_resource(resource);
        }
        game_data
    }

    /// Add a resource to the data structure
    fn add_resource(&mut self, resource: GameResource) {
        let id = self.resources.len();
        self.filename_index.insert(resource.filename.clone(), id);
        self.name_type_index
            .insert((resource.name.clone(), resource.r#type), id);
        self.resources.push(resource);
    }
}

/// A game resource
#[derive(Debug)]
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
    /// Where the resource is loaded
    pub data_origin: DataOrigin,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DataOrigin {
    Bif { name: String },
    Override { path: PathBuf },
    Missing,
}

impl GameResource {
    pub fn import(&self) -> io::Result<crate::imported_resource::ImportedResource> {
        use crate::imported_resource::ImportedResource;
        use infinitier_bam_importer::BamImporter;
        use infinitier_bmp_importer::BmpImporter;
        use infinitier_ids_importer::IdsImporter;
        use infinitier_ini_importer::IniImporter;
        use infinitier_pvr_importer::PvrzImporter;
        use infinitier_two_da_importer::TwoDAImporter;
        use infinitier_wed_importer::WedImporter;

        let ds = self
            .datasource
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no datasource available"))?;
        match self.r#type {
            ResourceType::Bam => BamImporter {}.import(ds).map(ImportedResource::Bam),
            ResourceType::Bmp => BmpImporter.import(ds).map(ImportedResource::Bmp),
            ResourceType::Ids => IdsImporter.import(ds).map(ImportedResource::Ids),
            ResourceType::Ini => IniImporter.import(ds).map(ImportedResource::Ini),
            ResourceType::Pvrz => PvrzImporter.import(ds).map(ImportedResource::Pvrz),
            ResourceType::TwoDA => TwoDAImporter.import(ds).map(ImportedResource::TwoDA),
            ResourceType::Wed => WedImporter.import(ds).map(ImportedResource::Wed),
            ResourceType::Acm => Ok(ImportedResource::Acm),
            ResourceType::Are => Ok(ImportedResource::Are),
            ResourceType::Bah => Ok(ImportedResource::Bah),
            ResourceType::Bcs => Ok(ImportedResource::Bcs),
            ResourceType::Bio => Ok(ImportedResource::Bio),
            ResourceType::Bs => Ok(ImportedResource::Bs),
            ResourceType::Chr => Ok(ImportedResource::Chr),
            ResourceType::Chu => Ok(ImportedResource::Chu),
            ResourceType::Cre => Ok(ImportedResource::Cre),
            ResourceType::Dlg => Ok(ImportedResource::Dlg),
            ResourceType::Eff => Ok(ImportedResource::Eff),
            ResourceType::Fnt => Ok(ImportedResource::Fnt),
            ResourceType::Gam => Ok(ImportedResource::Gam),
            ResourceType::Glsl => Ok(ImportedResource::Glsl),
            ResourceType::Gui => Ok(ImportedResource::Gui),
            ResourceType::Itm => Ok(ImportedResource::Itm),
            ResourceType::Lua => Ok(ImportedResource::Lua),
            ResourceType::Maze => Ok(ImportedResource::Maze),
            ResourceType::Menu => Ok(ImportedResource::Menu),
            ResourceType::Mos => Ok(ImportedResource::Mos),
            ResourceType::Mus => Ok(ImportedResource::Mus),
            ResourceType::Mve => Ok(ImportedResource::Mve),
            ResourceType::Plt => Ok(ImportedResource::Plt),
            ResourceType::Png => Ok(ImportedResource::Png),
            ResourceType::Pro => Ok(ImportedResource::Pro),
            ResourceType::Spl => Ok(ImportedResource::Spl),
            ResourceType::Sql => Ok(ImportedResource::Sql),
            ResourceType::Src => Ok(ImportedResource::Src),
            ResourceType::Sto => Ok(ImportedResource::Sto),
            ResourceType::Tga => Ok(ImportedResource::Tga),
            ResourceType::Tis => Ok(ImportedResource::Tis),
            ResourceType::Toh => Ok(ImportedResource::Toh),
            ResourceType::Tot => Ok(ImportedResource::Tot),
            ResourceType::Ttf => Ok(ImportedResource::Ttf),
            ResourceType::Vef => Ok(ImportedResource::Vef),
            ResourceType::Vvc => Ok(ImportedResource::Vvc),
            ResourceType::Wav => Ok(ImportedResource::Wav),
            ResourceType::Wbm => Ok(ImportedResource::Wbm),
            ResourceType::Wfx => Ok(ImportedResource::Wfx),
            ResourceType::Wmp => Ok(ImportedResource::Wmp),
            ResourceType::Unknown(id) => Ok(ImportedResource::Unknown(id)),
        }
    }
}

/// A game data builder
pub struct GameDataBuilder {
    /// File system root
    root: PathBuf,
    /// File system
    fs: CaseInsensitiveFS,
    /// Name of the key file
    key_file: String,
    /// Resource overrides folders
    overrides: Vec<String>,
    /// Game Type
    game_type: Game,
}

impl GameDataBuilder {
    /// Create a new game data builder
    pub fn new<P: AsRef<Path>>(game_root: P, game_type: Game) -> io::Result<GameDataBuilder> {
        Ok(GameDataBuilder {
            root: game_root.as_ref().to_path_buf(),
            game_type,
            fs: CaseInsensitiveFS::new_with_fallback(
                game_root,
                vec![
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
            )?,
            overrides: vec!["override".to_string()],
            key_file: "chitin.key".to_string(),
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
    pub fn with_fallbacks(mut self, fallbacks: Vec<String>) -> io::Result<GameDataBuilder> {
        self.fs = CaseInsensitiveFS::new_with_fallback(&self.root, fallbacks)?;
        Ok(self)
    }

    /// Set the resource override folders
    /// Default: ["override"]
    pub fn with_overrides(mut self, overrides: Vec<String>) -> GameDataBuilder {
        self.overrides = overrides;
        self
    }

    /// Build the game data
    pub fn build(&self) -> io::Result<GameData> {
        let mut game_data = GameData {
            game_type: self.game_type,
            resources: vec![],
            filename_index: HashMap::new(),
            name_type_index: HashMap::new(),
        };

        let key_path = self
            .fs
            .get_path(&CaseInsensitivePath::new(&self.key_file))?;
        let key = KeyImporter {}.import(&DataSource::new(key_path.as_path()))?;

        // preload all bif files
        let mut bif_all = vec![];
        for bif_entry in key.bif_entries {
            if let Some(bif_path) = self
                .fs
                .search_path_opt(&CaseInsensitivePath::new(&bif_entry.file_name))
            {
                let bif = BifImporter {
                    name: bif_entry.file_name,
                }
                .import(&DataSource::new(bif_path.as_path()))
                .unwrap();
                bif_all.push(Some(bif));
            } else {
                warn!("Bif file {} not found", bif_entry.file_name);
                bif_all.push(None);
            }
        }

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
                game_data.add_resource(GameResource {
                    name,
                    r#type,
                    filename,
                    file_size,
                    datasource,
                    data_origin: DataOrigin::Override { path: r#override },
                });
            } else {
                if let Some(Some(bif)) = bif_all.get(resource.bif_entries_index as usize) {
                    let bif_ds = bif.datasource.clone();
                    let bif_ds_clone = bif_ds.clone();
                    let bif_source: Arc<dyn Fn(u64, u64) -> DataSource + Send + Sync> =
                        Arc::new(move |offset, size| {
                            bif_ds_clone.clone().with_offset(offset, Some(size))
                        });

                    let key_locator = resource.bif_resource_locator;
                    let bif_resource = if resource.r#type == ResourceType::Tis {
                        bif.resources.iter().find(|r| {
                            matches!(r,
                                BifEmbeddedResource::Tileset { locator, .. }
                                if (*locator & 0xFC000) == (key_locator & 0xFC000)
                            )
                        })
                    } else {
                        bif.resources.iter().find(|r| {
                            matches!(r,
                                BifEmbeddedResource::File { locator, .. }
                                if (*locator & 0x3FFF) == (key_locator & 0x3FFF)
                            )
                        })
                    };
                    if let Some(bif_resource) = bif_resource {
                        debug!("Resource {} found in bif {:?}", filename, bif_ds);

                        let (datasource, file_size) = match bif_resource {
                            BifEmbeddedResource::File {
                                locator: _,
                                size,
                                offset,
                                r#type: _,
                            } => (bif_source(*offset, *size as u64), *size as u64),
                            BifEmbeddedResource::Tileset {
                                locator: _,
                                size,
                                count: _,
                                offset,
                                r#type: _,
                            } => (bif_source(*offset, *size as u64), *size as u64),
                        };

                        game_data.add_resource(GameResource {
                            name,
                            r#type,
                            filename,
                            file_size: Some(file_size),
                            datasource: Some(datasource),
                            data_origin: DataOrigin::Bif {
                                name: bif.name.clone(),
                            },
                        });
                    } else {
                        warn!("Resource {} not found in bif {:?}", filename, bif_ds);
                        game_data.add_resource(GameResource {
                            name,
                            r#type,
                            filename,
                            file_size: None,
                            datasource: None,
                            data_origin: DataOrigin::Missing,
                        });
                    }
                } else {
                    warn!("Resource {} not found", filename);
                    game_data.add_resource(GameResource {
                        name,
                        r#type,
                        filename,
                        file_size: None,
                        datasource: None,
                        data_origin: DataOrigin::Missing,
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
    use infinitier_two_da_importer::TwoDAImporter;
    use infinitier_wed_importer::WedImporter;

    use super::*;

    fn build_bg2() -> GameData {
        let bg_root = get_assets_path().join(BG2_RESOURCES_DIR.0);
        GameDataBuilder::new(bg_root, Game::Bg2).unwrap().build().unwrap()
    }

    #[test]
    fn test_game_data_builder() {
        let game_data = build_bg2();
        let key = KeyImporter {}
            .import(&DataSource::new(
                get_assets_path().join(BG2_RESOURCES_DIR.0).join("CHITIN.KEY"),
            ))
            .unwrap();
        assert_eq!(game_data.resources.len(), key.resource_entries.len());
    }

    #[test]
    fn test_resource_found() {
        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("AR0714", ResourceType::Wed)
            .unwrap();
        assert_eq!(
            DataOrigin::Bif {
                name: "data/area070c.bif".to_string()
            },
            resource.data_origin
        );

        // The data is into the assets/bg2/data/Data/AREA070C.bif file
        assert!(resource.datasource.is_some());

        // Test that the data can be read
        WedImporter
            .import(resource.datasource.as_ref().unwrap())
            .unwrap();
    }

    #[test]
    fn test_tis_resource_found() {
        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("AR0714", ResourceType::Tis)
            .unwrap();
        assert_eq!(
            DataOrigin::Bif {
                name: "data/area070c.bif".to_string()
            },
            resource.data_origin
        );

        // The data is into the assets/bg2/data/Data/AREA070C.bif file
        assert!(resource.datasource.is_some());

        // ToDo: implement when tis importer is available
        // Test that the data can be read
        // WedImporter::import(resource.datasource.as_ref().unwrap()).unwrap();
    }

    #[test]
    fn test_resource_found_in_override() {
        let game_data = build_bg2();

        let resource = game_data
            .get_by_name_and_type("ABCLASRQ", ResourceType::TwoDA)
            .unwrap();
        let path = get_assets_path()
            .join(BG2_RESOURCES_DIR.0)
            .join("override/AbClasRq.2DA");
        assert_eq!(DataOrigin::Override { path }, resource.data_origin);

        // Test that the override datasource can be read
        TwoDAImporter
            .import(resource.datasource.as_ref().unwrap())
            .unwrap();
    }

    #[test]
    fn test_get_by_id_found() {
        let game_data = build_bg2();
        let resource = game_data.get_by_id(0).unwrap();
        assert_eq!(resource.name, "ABCLASRQ");
        assert_eq!(resource.r#type, ResourceType::TwoDA);
        assert_eq!(resource.filename, "abclasrq.2da");

        let path = get_assets_path()
            .join(BG2_RESOURCES_DIR.0)
            .join("override/AbClasRq.2DA");
        assert_eq!(DataOrigin::Override { path }, resource.data_origin);
    }

    #[test]
    fn test_get_by_id_not_found() {
        let game_data = build_bg2();
        assert!(game_data.get_by_id(game_data.resources.len()).is_none());
    }

    #[test]
    fn test_get_by_filename_found() {
        let game_data = build_bg2();
        let resource = game_data.get_by_filename("abclasrq.2da").unwrap();
        assert_eq!(resource.name, "ABCLASRQ");
        assert_eq!(resource.r#type, ResourceType::TwoDA);
    }

    #[test]
    fn test_get_by_filename_not_found() {
        let game_data = build_bg2();
        assert!(game_data.get_by_filename("nonexistent.bam").is_none());
    }

    #[test]
    fn test_get_by_name_and_type_found() {
        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("ABDCDSRQ", ResourceType::TwoDA)
            .unwrap();
        assert_eq!(resource.filename, "abdcdsrq.2da");
        assert!(resource.datasource.is_none());
        assert!(resource.file_size.is_none());
        assert_eq!(DataOrigin::Missing, resource.data_origin);
    }

    #[test]
    fn test_get_by_name_and_type_not_found() {
        let game_data = build_bg2();
        assert!(
            game_data
                .get_by_name_and_type("ABCLASRQ", ResourceType::Bam)
                .is_none()
        );
    }
}
