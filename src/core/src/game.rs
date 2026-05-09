use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use infinitier_acm_decoder::AcmDecoder;
use infinitier_bif_importer::{BifEmbeddedResource, BifImporter};
use infinitier_common::{Game, ResourceType};
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CiPath};
use infinitier_key_importer::KeyImporter;
use infinitier_wav_decoder::WavDecoder;
use log::{debug, warn};

use crate::sound::SoundDecoder;

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

    /// Return the game type
    pub fn game(&self) -> Game {
        self.game_type
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

    /// Add a resource to the data structure.
    /// If a resource with the same name and type already exists, it is replaced.
    fn add_resource(&mut self, resource: GameResource) {
        let key = (resource.name.clone(), resource.r#type);
        if let Some(&existing_id) = self.name_type_index.get(&key) {
            let old_filename = self.resources[existing_id].filename.clone();
            if old_filename != resource.filename {
                self.filename_index.remove(&old_filename);
                self.filename_index
                    .insert(resource.filename.clone(), existing_id);
            }
            self.resources[existing_id] = resource;
        } else {
            let id = self.resources.len();
            self.filename_index.insert(resource.filename.clone(), id);
            self.name_type_index.insert(key, id);
            self.resources.push(resource);
        }
    }
}

/// A game resource
#[derive(Debug)]
pub struct GameResource {
    /// Game type
    pub game_type: Game,
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
    Dir { name: String, path: PathBuf },
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
            ResourceType::Acm => Ok(AcmDecoder::open(
                ds,
                infinitier_acm_decoder::OutputChannels::Original,
                &self.name,
            )
            .map(SoundDecoder::Acm)
            .map(ImportedResource::Sound)?),
            ResourceType::Wav => Ok(WavDecoder::open(ds, &self.name)
                .map(SoundDecoder::Wav)
                .map(ImportedResource::Sound)?),
            ResourceType::Bam => BamImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Bam),
            ResourceType::Bmp => BmpImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Bmp),
            ResourceType::Ids => IdsImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ids),
            ResourceType::Ini => IniImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ini),
            ResourceType::Pvrz => PvrzImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Pvrz),
            ResourceType::TwoDA => TwoDAImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::TwoDA),
            ResourceType::Wed => WedImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Wed),
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
            ResourceType::Mve => Ok(ImportedResource::Mve(crate::movie::MovieSource::new(
                ds.clone(),
                &self.name,
            ))),
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
            ResourceType::Wbm => Ok(ImportedResource::Wbm(crate::movie::MovieSource::new(
                ds.clone(),
                &self.name,
            ))),
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
            .get_path(&CiPath::new(&self.key_file))?;
        let key = KeyImporter {
            name: &self.key_file,
        }
        .import(&DataSource::new(key_path.as_path()))?;

        // Additional resources are loaded from hardcoded paths (i.e. Scripts, Musics, etc.)

        // preload all bif files
        let mut bif_all = vec![];
        for bif_entry in key.bif_entries {
            if let Some(bif_path) = self
                .fs
                .search_path_opt(&CiPath::new(&bif_entry.file_name))
            {
                let bif = BifImporter {
                    name: &bif_entry.file_name,
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
            let cs_path = CiPath::new(filename);
            let filename = cs_path.base_name().to_string();


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
                            game_type: self.game_type,
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
                            game_type: self.game_type,
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
                        game_type: self.game_type,
                        name,
                        r#type,
                        filename,
                        file_size: None,
                        datasource: None,
                        data_origin: DataOrigin::Missing,
                    });
                }
        }

        self.add_resources_from_dir(&mut game_data, "characters", ResourceType::Bio.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "characters", ResourceType::Chr.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "data", ResourceType::Mve.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "movies", ResourceType::Wbm.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "music", ResourceType::Acm.get_extension(), true)?;
        self.add_resources_from_dir(&mut game_data, "music", ResourceType::Mus.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "scripts", ResourceType::Bs.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "sounds", ResourceType::Wav.get_extension(), false)?;
        self.add_resources_from_dir(&mut game_data, "override", None, false)?;

        Ok(game_data)
    }

    fn add_resources_from_dir(&self, game: &mut GameData, dir_name: &str, extension: Option<&str>, recursive: bool) -> io::Result<()> {
        debug!("Searching for resources in {}/{:?}", dir_name, extension);
        for resource in
            self.fs
                .list_files(&CiPath::new(dir_name), extension, recursive)
        {
            let name = resource.file_stem().unwrap_or_default().to_str().unwrap_or_default().to_lowercase();
            let extension = resource.extension().unwrap_or_default().to_str().unwrap_or_default().to_lowercase();

            debug!("Found resource {}", resource.display());
            game.add_resource(GameResource {
                data_origin: DataOrigin::Dir {
                    name: dir_name.to_owned(),
                    path: resource.clone(),
                },
                file_size: Some(resource.metadata()?.len()),
                datasource: Some(DataSource::new(resource.as_path())),
                game_type: self.game_type,
                r#type: ResourceType::from_extension(&extension).unwrap_or(ResourceType::Unknown(0)),
                name,
                filename: resource.file_name().unwrap_or_default().to_str().unwrap_or_default().to_lowercase(),
            });
        }
        Ok(())
    }

}


#[cfg(test)]
mod tests {
    use infinitier_test_utils::{constants::BG2_RESOURCES_DIR, get_assets_path};
    use infinitier_two_da_importer::TwoDAImporter;
    use infinitier_wed_importer::WedImporter;

    use super::*;

    fn build_bg2() -> GameData {
        let bg_root = get_assets_path().join("KEY").join(BG2_RESOURCES_DIR.0);
        GameDataBuilder::new(bg_root, Game::Bg2)
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn test_game_data_builder() {
        let game_data = build_bg2();
        let key = KeyImporter { name: "chitin.key" }
            .import(&DataSource::new(
                get_assets_path()
                    .join("KEY")
                    .join(BG2_RESOURCES_DIR.0)
                    .join("CHITIN.KEY"),
            ))
            .unwrap();
        assert!(!game_data.is_empty());
        assert!(game_data.resources.len() <= key.resource_entries.len());
    }

    #[test]
    fn test_resource_found() {
        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("ar0714", ResourceType::Wed)
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
        WedImporter { name: "ar0714" }
            .import(resource.datasource.as_ref().unwrap())
            .unwrap();
    }

    #[test]
    fn test_tis_resource_found() {
        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("ar0714", ResourceType::Tis)
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
            .get_by_name_and_type("abclasrq", ResourceType::TwoDA)
            .unwrap();
        let path = get_assets_path()
            .join("KEY")
            .join(BG2_RESOURCES_DIR.0)
            .join("override/AbClasRq.2DA");
        assert_eq!(DataOrigin::Dir { name: "override".to_owned(), path }, resource.data_origin);

        // Test that the override datasource can be read
        TwoDAImporter { name: "abclasrq" }
            .import(resource.datasource.as_ref().unwrap())
            .unwrap();
    }

    #[test]
    fn test_get_by_id_found() {
        let game_data = build_bg2();
        let resource = game_data.get_by_id(0).unwrap();
        assert_eq!(resource.name, "abclasrq");
        assert_eq!(resource.r#type, ResourceType::TwoDA);
        assert_eq!(resource.filename, "abclasrq.2da");

        let path = get_assets_path()
            .join("KEY")
            .join(BG2_RESOURCES_DIR.0)
            .join("override/AbClasRq.2DA");
        assert_eq!(DataOrigin::Dir { name: "override".to_owned(), path }, resource.data_origin);
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
        assert_eq!(resource.name, "abclasrq");
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
            .get_by_name_and_type("abdcdsrq", ResourceType::TwoDA)
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
                .get_by_name_and_type("abclasrq", ResourceType::Bam)
                .is_none()
        );
    }

    fn make_resource(
        name: &str,
        r#type: ResourceType,
        filename: &str,
        origin: DataOrigin,
    ) -> GameResource {
        GameResource {
            game_type: Game::Bg2,
            name: name.to_string(),
            r#type,
            filename: filename.to_string(),
            file_size: None,
            datasource: None,
            data_origin: origin,
        }
    }

    #[test]
    fn test_add_resource_replaces_when_name_and_type_match() {
        let mut game_data = GameData::new(vec![], Game::Bg2);

        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
            "test.bam",
            DataOrigin::Bif {
                name: "first.bif".to_string(),
            },
        ));
        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
            "test.bam",
            DataOrigin::Bif {
                name: "second.bif".to_string(),
            },
        ));

        assert_eq!(game_data.len(), 1);

        let by_name = game_data
            .get_by_name_and_type("TEST", ResourceType::Bam)
            .unwrap();
        assert_eq!(
            by_name.data_origin,
            DataOrigin::Bif {
                name: "second.bif".to_string()
            }
        );

        let by_filename = game_data.get_by_filename("test.bam").unwrap();
        assert_eq!(
            by_filename.data_origin,
            DataOrigin::Bif {
                name: "second.bif".to_string()
            }
        );

        let by_id = game_data.get_by_id(0).unwrap();
        assert_eq!(
            by_id.data_origin,
            DataOrigin::Bif {
                name: "second.bif".to_string()
            }
        );
    }

    #[test]
    fn test_add_resource_replace_updates_filename_index_when_filename_differs() {
        let mut game_data = GameData::new(vec![], Game::Bg2);

        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
            "old.bam",
            DataOrigin::Missing,
        ));
        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
            "new.bam",
            DataOrigin::Missing,
        ));

        assert_eq!(game_data.len(), 1);
        assert!(game_data.get_by_filename("old.bam").is_none());
        let by_filename = game_data.get_by_filename("new.bam").unwrap();
        assert_eq!(by_filename.name, "TEST");
    }

    #[test]
    fn test_add_resource_keeps_existing_when_type_differs() {
        let mut game_data = GameData::new(vec![], Game::Bg2);

        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
            "test.bam",
            DataOrigin::Missing,
        ));
        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Wed,
            "test.wed",
            DataOrigin::Missing,
        ));

        assert_eq!(game_data.len(), 2);
        assert!(
            game_data
                .get_by_name_and_type("TEST", ResourceType::Bam)
                .is_some()
        );
        assert!(
            game_data
                .get_by_name_and_type("TEST", ResourceType::Wed)
                .is_some()
        );
    }

    #[test]
    fn test_add_resources_from_dir_lowercases_name_and_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        std::fs::create_dir(root.join("MUSIC")).unwrap();
        std::fs::File::create(root.join("MUSIC/MyTune.MUS")).unwrap();
        std::fs::File::create(root.join("MUSIC/Other.mus")).unwrap();
        std::fs::File::create(root.join("MUSIC/THIRD.Mus")).unwrap();
        // A non-matching extension to make sure the filter still works
        std::fs::File::create(root.join("MUSIC/notes.txt")).unwrap();

        let builder = GameDataBuilder::new(root, Game::Bg2).unwrap();
        let mut game_data = GameData::new(vec![], Game::Bg2);

        builder
            .add_resources_from_dir(&mut game_data, "music", Some("mus"), false)
            .unwrap();

        assert_eq!(game_data.len(), 3);

        for (orig_filename, expected_name) in [
            ("MyTune.MUS", "mytune"),
            ("Other.mus", "other"),
            ("THIRD.Mus", "third"),
        ] {
            let lower_filename = orig_filename.to_ascii_lowercase();
            let resource = game_data
                .get_by_filename(&lower_filename)
                .unwrap_or_else(|| panic!("resource {lower_filename} not found"));
            assert_eq!(resource.name, expected_name);
            assert_eq!(resource.filename, lower_filename);
            assert_eq!(resource.r#type, ResourceType::Mus);

            // get_by_name_and_type uses the stored (lowercased) name
            assert!(
                game_data
                    .get_by_name_and_type(expected_name, ResourceType::Mus)
                    .is_some()
            );
        }
    }

    #[test]
    fn test_add_resources_from_dir_no_extension_filter_infers_type() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        std::fs::create_dir(root.join("OVERRIDE")).unwrap();
        std::fs::File::create(root.join("OVERRIDE/Foo.BAM")).unwrap();
        std::fs::File::create(root.join("OVERRIDE/Bar.WED")).unwrap();
        std::fs::File::create(root.join("OVERRIDE/Baz.UNKNOWNEXT")).unwrap();

        let builder = GameDataBuilder::new(root, Game::Bg2).unwrap();
        let mut game_data = GameData::new(vec![], Game::Bg2);

        builder
            .add_resources_from_dir(&mut game_data, "override", None, false)
            .unwrap();

        assert_eq!(game_data.len(), 3);

        let foo = game_data.get_by_filename("foo.bam").unwrap();
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.r#type, ResourceType::Bam);

        let bar = game_data.get_by_filename("bar.wed").unwrap();
        assert_eq!(bar.name, "bar");
        assert_eq!(bar.r#type, ResourceType::Wed);

        let baz = game_data.get_by_filename("baz.unknownext").unwrap();
        assert_eq!(baz.name, "baz");
        assert_eq!(baz.r#type, ResourceType::Unknown(0));
    }

    #[test]
    fn test_add_resource_keeps_existing_when_name_differs() {
        let mut game_data = GameData::new(vec![], Game::Bg2);

        game_data.add_resource(make_resource(
            "TEST1",
            ResourceType::Bam,
            "test1.bam",
            DataOrigin::Missing,
        ));
        game_data.add_resource(make_resource(
            "TEST2",
            ResourceType::Bam,
            "test2.bam",
            DataOrigin::Missing,
        ));

        assert_eq!(game_data.len(), 2);
        assert!(
            game_data
                .get_by_name_and_type("TEST1", ResourceType::Bam)
                .is_some()
        );
        assert!(
            game_data
                .get_by_name_and_type("TEST2", ResourceType::Bam)
                .is_some()
        );
    }
}
