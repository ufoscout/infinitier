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
use infinitier_key_resource::KeyImporter;
use infinitier_wav_decoder::WavDecoder;
use log::{debug, warn};

use crate::imported_resource::{movie, sound::SoundDecoder};

pub type ResourceId = usize;

/// The Data of a game.
#[derive(Debug)]
pub struct GameData {
    /// Game Type
    game_type: Game,
    /// All resources
    resources: Vec<GameResource>,
    /// A map from (name, type) to resource id
    name_type_index: HashMap<(String, ResourceType), ResourceId>,
    /// A map from type to every resource id of that type.
    type_index: HashMap<ResourceType, Vec<ResourceId>>,
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

    /// Get a resource by name and type
    pub fn get_by_name_and_type(&self, name: &str, r#type: ResourceType) -> Option<&GameResource> {
        self.name_type_index
            .get(&(name.to_string(), r#type))
            .and_then(|&id| self.resources.get(id))
    }

    /// Return every resource of `r#type`. Lookup is constant-time via
    /// the pre-built type index; iteration is then linear in the number
    /// of matches. Yields nothing when no resource of that type exists.
    pub fn get_all_by_type(&self, r#type: ResourceType) -> impl Iterator<Item = &GameResource> {
        self.type_index
            .get(&r#type)
            .into_iter()
            .flat_map(move |ids| ids.iter().filter_map(move |&id| self.resources.get(id)))
    }

    /// Creates a GameData from a list of resources
    pub fn new(resources: Vec<GameResource>, game_type: Game) -> Self {
        let mut game_data = GameData {
            game_type,
            resources: Vec::new(),
            name_type_index: HashMap::new(),
            type_index: HashMap::new(),
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
            // Same (name, type) → replacing in place. The id is already
            // present in `type_index[type]`, no index update needed.
            self.resources[existing_id] = resource;
        } else {
            let id = self.resources.len();
            self.name_type_index.insert(key, id);
            self.type_index.entry(resource.r#type).or_default().push(id);
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
    /// File size
    pub file_size: Option<u64>,
    /// Data source
    pub datasource: Option<DataSource>,
    /// Where the resource is loaded
    pub data_origin: DataOrigin,
}

impl GameResource {
    pub fn resource_name_with_extension(&self) -> String {
        if let Some(extension) = self.r#type.get_extension() {
            format!("{}.{}", self.name, extension)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DataOrigin {
    Bif { name: String },
    Dir { name: String, path: CiPath },
    Missing,
}

impl GameResource {
    pub fn import(
        &self,
        game_data: &GameData,
    ) -> io::Result<crate::imported_resource::ImportedResource> {
        use crate::imported_resource::{
            ImportedResource, bam::ImportedBam, bcs::ImportedBcs, image::ImportedImage,
        };
        use infinitier_bam_importer::BamImporter;
        use infinitier_bcs_resource::BcsImporter;
        use infinitier_bmp_resource::BmpImporter;
        use infinitier_ids_resource::IdsImporter;
        use infinitier_ini_resource::IniImporter;
        use infinitier_pvrz_resource::PvrzImporter;
        use infinitier_two_da_resource::TwoDAImporter;
        use infinitier_wed_resource::WedImporter;

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
                .and_then(|bam| ImportedBam::load(bam, game_data))
                .map(ImportedResource::Bam),
            // BMP and PVRZ both feed into the unified `ImportedImage`
            // wrapper so the explorer can render them through one
            // `ImageViewer`. Adding new raster formats here only needs a
            // new `ImportedImage::from_*` constructor.
            ResourceType::Bmp => BmpImporter { name: &self.name }
                .import(ds)
                .map(ImportedImage::from_bmp)
                .map(ImportedResource::Image),
            ResourceType::Pvrz => PvrzImporter { name: &self.name }
                .import(ds)
                .and_then(|header| ImportedImage::from_pvrz(header, ds))
                .map(ImportedResource::Image),
            ResourceType::Ids => IdsImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ids),
            ResourceType::Ini => IniImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ini),
            ResourceType::TwoDA => TwoDAImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::TwoDA),
            ResourceType::Wed => WedImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Wed),
            ResourceType::Are => Ok(ImportedResource::Are),
            ResourceType::Bah => Ok(ImportedResource::Bah),
            // BCS and BS share the same bytecode format — BS is just the
            // saved-game flavour with a different extension. Route both
            // through the same importer + `ImportedBcs::load` pipeline so
            // the explorer's `BcsViewer` renders them identically.
            ResourceType::Bcs | ResourceType::Bs => BcsImporter { name: &self.name }
                .import(ds)
                .and_then(|bcs| ImportedBcs::load(bcs, game_data))
                .map(ImportedResource::Bcs),
            ResourceType::Bio => Ok(ImportedResource::Bio),
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
            ResourceType::Mve => Ok(ImportedResource::Mve(movie::MovieSource::new(
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
            ResourceType::Wbm => Ok(ImportedResource::Wbm(movie::MovieSource::new(
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
            name_type_index: HashMap::new(),
            type_index: HashMap::new(),
        };

        let key_path = self.fs.get_path(&self.key_file)?;
        let key = KeyImporter {
            name: &self.key_file,
        }
        .import(&DataSource::new(key_path.path()))?;

        // Additional resources are loaded from hardcoded paths (i.e. Scripts, Musics, etc.)

        // preload all bif files
        let mut bif_all = vec![];
        for bif_entry in key.bif_entries {
            if let Some(bif_path) = self.fs.search_path_opt(&bif_entry.file_name) {
                let bif = BifImporter {
                    name: &bif_entry.file_name,
                }
                .import(&DataSource::new(bif_path.path()))
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
                    debug!("Resource {}.{:?} found in bif {:?}", name, r#type, bif_ds);

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
                        file_size: Some(file_size),
                        datasource: Some(datasource),
                        data_origin: DataOrigin::Bif {
                            name: bif.name.clone(),
                        },
                    });
                } else {
                    warn!(
                        "Resource {}.{:?} not found in bif {:?}",
                        name, r#type, bif_ds
                    );
                    game_data.add_resource(GameResource {
                        game_type: self.game_type,
                        name,
                        r#type,
                        file_size: None,
                        datasource: None,
                        data_origin: DataOrigin::Missing,
                    });
                }
            } else {
                warn!("Resource {}.{:?} not found", name, r#type);
                game_data.add_resource(GameResource {
                    game_type: self.game_type,
                    name,
                    r#type,
                    file_size: None,
                    datasource: None,
                    data_origin: DataOrigin::Missing,
                });
            }
        }

        self.add_resources_from_dir(
            &mut game_data,
            "characters",
            ResourceType::Bio.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "characters",
            ResourceType::Chr.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "data",
            ResourceType::Mve.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "movies",
            ResourceType::Wbm.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "music",
            ResourceType::Acm.get_extension(),
            true,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "music",
            ResourceType::Mus.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "scripts",
            ResourceType::Bs.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(
            &mut game_data,
            "sounds",
            ResourceType::Wav.get_extension(),
            false,
        )?;
        self.add_resources_from_dir(&mut game_data, "override", None, false)?;

        Ok(game_data)
    }

    fn add_resources_from_dir(
        &self,
        game: &mut GameData,
        dir_name: &str,
        extension: Option<&str>,
        recursive: bool,
    ) -> io::Result<()> {
        debug!("Searching for resources in {}/{:?}", dir_name, extension);
        for resource in self.fs.list_files(dir_name, extension, recursive) {
            let real = resource.path();
            let name = resource.base_name_without_extension().to_string();
            let r#type = resource
                .extension()
                .and_then(ResourceType::from_extension)
                .unwrap_or(ResourceType::Unknown(0));
            let file_size = Some(real.metadata()?.len());
            let datasource = Some(DataSource::new(real));

            debug!("Found resource {}", real.display());
            game.add_resource(GameResource {
                data_origin: DataOrigin::Dir {
                    name: dir_name.to_owned(),
                    path: resource,
                },
                file_size,
                datasource,
                game_type: self.game_type,
                r#type,
                name,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use infinitier_test_utils::{constants::BG2_RESOURCES_DIR, get_assets_path};
    use infinitier_two_da_resource::TwoDAImporter;
    use infinitier_wed_resource::WedImporter;

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
        let expected_path = get_assets_path()
            .join("KEY")
            .join(BG2_RESOURCES_DIR.0)
            .join("override/AbClasRq.2DA");
        let DataOrigin::Dir { name, path } = &resource.data_origin else {
            panic!("expected DataOrigin::Dir, got {:?}", resource.data_origin);
        };
        assert_eq!(name, "override");
        assert_eq!(path.path(), expected_path.as_path());

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
        assert_eq!(resource.resource_name_with_extension(), "abclasrq.2da");

        let expected_path = get_assets_path()
            .join("KEY")
            .join(BG2_RESOURCES_DIR.0)
            .join("override/AbClasRq.2DA");
        let DataOrigin::Dir { name, path } = &resource.data_origin else {
            panic!("expected DataOrigin::Dir, got {:?}", resource.data_origin);
        };
        assert_eq!(name, "override");
        assert_eq!(path.path(), expected_path.as_path());
    }

    #[test]
    fn test_get_by_id_not_found() {
        let game_data = build_bg2();
        assert!(game_data.get_by_id(game_data.resources.len()).is_none());
    }

    #[test]
    fn test_get_by_name_and_type_found() {
        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("abdcdsrq", ResourceType::TwoDA)
            .unwrap();
        assert_eq!(resource.resource_name_with_extension(), "abdcdsrq.2da");
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

    fn make_resource(name: &str, r#type: ResourceType, origin: DataOrigin) -> GameResource {
        GameResource {
            game_type: Game::Bg2,
            name: name.to_string(),
            r#type,
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
            DataOrigin::Bif {
                name: "first.bif".to_string(),
            },
        ));
        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
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

        let by_id = game_data.get_by_id(0).unwrap();
        assert_eq!(
            by_id.data_origin,
            DataOrigin::Bif {
                name: "second.bif".to_string()
            }
        );
    }

    #[test]
    fn test_add_resource_keeps_existing_when_type_differs() {
        let mut game_data = GameData::new(vec![], Game::Bg2);

        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Bam,
            DataOrigin::Missing,
        ));
        game_data.add_resource(make_resource(
            "TEST",
            ResourceType::Wed,
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
                .get_by_name_and_type(expected_name, ResourceType::Mus)
                .unwrap_or_else(|| panic!("resource {lower_filename} not found"));
            assert_eq!(resource.name, expected_name);
            assert_eq!(resource.resource_name_with_extension(), lower_filename);
            assert_eq!(resource.r#type, ResourceType::Mus);
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

        let foo = game_data
            .get_by_name_and_type("foo", ResourceType::Bam)
            .unwrap();
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.r#type, ResourceType::Bam);

        let bar = game_data
            .get_by_name_and_type("bar", ResourceType::Wed)
            .unwrap();
        assert_eq!(bar.name, "bar");
        assert_eq!(bar.r#type, ResourceType::Wed);

        let baz = game_data
            .get_by_name_and_type("baz", ResourceType::Unknown(0))
            .unwrap();
        assert_eq!(baz.name, "baz");
        assert_eq!(baz.r#type, ResourceType::Unknown(0));
    }

    #[test]
    fn test_add_resource_keeps_existing_when_name_differs() {
        let mut game_data = GameData::new(vec![], Game::Bg2);

        game_data.add_resource(make_resource(
            "TEST1",
            ResourceType::Bam,
            DataOrigin::Missing,
        ));
        game_data.add_resource(make_resource(
            "TEST2",
            ResourceType::Bam,
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

    #[test]
    fn test_get_all_by_type() {
        let mut game_data = GameData::new(vec![], Game::Bg2);
        game_data.add_resource(make_resource("A", ResourceType::Bam, DataOrigin::Missing));
        game_data.add_resource(make_resource("B", ResourceType::Wed, DataOrigin::Missing));
        game_data.add_resource(make_resource("C", ResourceType::Bam, DataOrigin::Missing));
        // Same name+type → replaces "A", does not duplicate the index entry.
        game_data.add_resource(make_resource(
            "A",
            ResourceType::Bam,
            DataOrigin::Bif {
                name: "data/replacement.bif".to_string(),
            },
        ));

        let bam_names: Vec<&str> = game_data
            .get_all_by_type(ResourceType::Bam)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(bam_names, vec!["A", "C"]);

        let wed_names: Vec<&str> = game_data
            .get_all_by_type(ResourceType::Wed)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(wed_names, vec!["B"]);

        // The replacement must be observable through the type index.
        let a = game_data
            .get_all_by_type(ResourceType::Bam)
            .find(|r| r.name == "A")
            .unwrap();
        assert_eq!(
            a.data_origin,
            DataOrigin::Bif {
                name: "data/replacement.bif".to_string(),
            }
        );

        // Unknown type → empty.
        assert_eq!(game_data.get_all_by_type(ResourceType::Tga).count(), 0);
    }
}
