use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Error},
    sync::Arc,
};

use infinitier_acm_resource::AcmImporter;
use infinitier_bif_resource::{BifEmbeddedResource, BifImporter};
use infinitier_common::{Engine, Game, ResourceType};
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CiPath, roots::Roots};
use infinitier_key_resource::KeyImporter;
use infinitier_wav_resource::WavImporter;
use log::{debug, error, warn};

use crate::{
    game_detect::detect_game,
    imported_resource::{ImportedResource, movie, sound::SoundDecoder},
};

pub type ResourceId = usize;

/// The Data of a game.
#[derive(Debug)]
pub struct GameData {
    /// File system
    fs: CaseInsensitiveFS,
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
    /// Return the underlying file system
    pub fn fs(&self) -> &CaseInsensitiveFS {
        &self.fs
    }

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
            .get(&(name.to_lowercase(), r#type))
            .and_then(|&id| self.resources.get(id))
    }

    /// Import a resource by name and type. Returns a [`io::ErrorKind::NotFound`]
    /// error when no resource with that name and type exists in the game data.
    pub fn import_by_name_and_type(
        &self,
        name: &str,
        r#type: ResourceType,
    ) -> io::Result<Cow<'_, ImportedResource>> {
        self.get_by_name_and_type(name, r#type)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "resource '{name}' of type {:?} not found in game data",
                        r#type
                    ),
                )
            })?
            .import(self)
    }

    /// Return every resource of `r#type`. Lookup is constant-time via
    /// the pre-built type index; iteration is then linear in the number
    /// of matches. Yields nothing when no resource of that type exists.
    pub fn get_all_resources_by_type(
        &self,
        r#type: ResourceType,
    ) -> impl Iterator<Item = &GameResource> {
        self.type_index
            .get(&r#type)
            .into_iter()
            .flat_map(move |ids| ids.iter().filter_map(move |&id| self.resources.get(id)))
    }

    /// Discover the save games visible on `fs`.
    pub fn save_games(&self) -> crate::save_games::SaveGames {
        crate::save_games::scan_save_games(&self.fs)
    }

    /// Locate and load the game's `dialog.tlk`.
    pub fn dialog_tlk(&self) -> io::Result<Cow<'_, infinitier_tlk_resource::Tlk>> {
        match self.import_by_name_and_type("dialog", ResourceType::Tlk)? {
            Cow::Borrowed(ImportedResource::Tlk(tlk)) => Ok(Cow::Borrowed(tlk)),
            Cow::Owned(ImportedResource::Tlk(tlk)) => Ok(Cow::Owned(tlk)),
            _ => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "dialog.tlk not found in game data",
            )),
        }
    }

    /// Import an IDS resource by name.
    pub fn import_ids_by_name(
        &self,
        name: &str,
    ) -> io::Result<Cow<'_, infinitier_ids_resource::Ids>> {
        match self.import_by_name_and_type(name, ResourceType::Ids)? {
            Cow::Borrowed(ImportedResource::Ids(ids)) => Ok(Cow::Borrowed(ids)),
            Cow::Owned(ImportedResource::Ids(ids)) => Ok(Cow::Owned(ids)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("resource '{name}' is not an IDS resource"),
            )),
        }
    }

    /// Import a 2DA resource by name, returning the parsed
    /// [`TwoDA`](infinitier_two_da_resource::TwoDA) (borrowed when cached).
    /// `NotFound` if the resource is absent.
    pub fn import_2da_by_name(
        &self,
        name: &str,
    ) -> io::Result<Cow<'_, infinitier_two_da_resource::TwoDA>> {
        match self.import_by_name_and_type(name, ResourceType::TwoDA)? {
            Cow::Borrowed(ImportedResource::TwoDA(two_da)) => Ok(Cow::Borrowed(two_da)),
            Cow::Owned(ImportedResource::TwoDA(two_da)) => Ok(Cow::Owned(two_da)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("resource '{name}' is not a 2DA resource"),
            )),
        }
    }

    /// Import a SPL resource by name, returning the parsed
    /// [`Spl`](infinitier_spl_resource::Spl) (borrowed when cached).
    /// `NotFound` if the resource is absent.
    pub fn import_spl_by_name(
        &self,
        name: &str,
    ) -> io::Result<Cow<'_, infinitier_spl_resource::Spl>> {
        match self.import_by_name_and_type(name, ResourceType::Spl)? {
            Cow::Borrowed(ImportedResource::Spl(spl)) => Ok(Cow::Borrowed(spl)),
            Cow::Owned(ImportedResource::Spl(spl)) => Ok(Cow::Owned(spl)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("resource '{name}' is not a SPL resource"),
            )),
        }
    }

    /// Import an ITM resource by name, returning the parsed
    /// [`Itm`](infinitier_itm_resource::Itm) (borrowed when cached).
    /// `NotFound` if the resource is absent.
    pub fn import_itm_by_name(
        &self,
        name: &str,
    ) -> io::Result<Cow<'_, infinitier_itm_resource::Itm>> {
        match self.import_by_name_and_type(name, ResourceType::Itm)? {
            Cow::Borrowed(ImportedResource::Itm(itm)) => Ok(Cow::Borrowed(itm)),
            Cow::Owned(ImportedResource::Itm(itm)) => Ok(Cow::Owned(itm)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("resource '{name}' is not an ITM resource"),
            )),
        }
    }

    /// Creates a GameData from a list of resources
    pub fn new(resources: Vec<GameResource>, game_type: Game, fs: CaseInsensitiveFS) -> Self {
        let mut game_data = GameData {
            fs,
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
        let key = (resource.name.to_lowercase(), resource.r#type);
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
    /// Data source for the resource
    pub datasource: Option<DataSource>,
    /// Where the resource is loaded
    pub data_origin: DataOrigin,
    /// Pre-imported resource. When `Some`, [`import`](Self::import) returns
    /// this cached value (borrowed) instead of performing a full import
    /// cycle from the [`datasource`](Self::datasource).
    pub imported: Option<ImportedResource>,
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

/// The text encoding of a game's `dialog.tlk` string section.
///
/// Enhanced Edition games (the [`Engine::Ee`] family — BG:EE / BG2:EE /
/// IWD:EE / PST:EE / EET / SoD) store the TLK as UTF-8. Classic (pre-EE)
/// releases use the WINDOWS-1252 single-byte code page for Western
/// locales — the importer's default — so they keep it. (Non-Western
/// classic locales use other legacy code pages; those aren't modelled
/// here yet.)
fn dialog_tlk_encoding(game: Game) -> &'static encoding_rs::Encoding {
    match game.engine() {
        Engine::Ee => encoding_rs::UTF_8,
        _ => encoding_rs::WINDOWS_1252,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DataOrigin {
    Bif { name: String },
    Dir { name: String, path: CiPath },
    Missing,
}

/// Build the 24-byte standalone-TIS header that a BIF-embedded tileset
/// is missing. The header is prepended (lazily, via
/// [`DataSource::new_concat`]) to the raw tile bytes still living inside
/// the BIF — neither side gets read into memory ahead of time.
///
/// Header layout (little-endian, matching IESDP):
/// - `"TIS V1  "` signature (8 bytes)
/// - `tile_count` (u32)
/// - `tile_length` (u32) — 5120 for palette, 12 for PVRZ
/// - `header_size` (u32) — fixed at 24
/// - `tile_dimension` (u32) — fixed at 64 in every Infinity Engine game
fn synthesise_tis_header(tile_count: u32, tile_length: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(b"TIS V1  ");
    bytes.extend_from_slice(&tile_count.to_le_bytes());
    bytes.extend_from_slice(&tile_length.to_le_bytes());
    bytes.extend_from_slice(&24u32.to_le_bytes());
    bytes.extend_from_slice(&64u32.to_le_bytes());
    bytes
}

impl GameResource {
    pub fn import(
        &self,
        game_data: &GameData,
    ) -> io::Result<Cow<'_, crate::imported_resource::ImportedResource>> {
        // Already imported: hand back the cached value borrowed, skipping the
        // full import cycle entirely.
        if let Some(imported) = &self.imported {
            return Ok(Cow::Borrowed(imported));
        }

        use crate::imported_resource::{
            ImportedResource, bam::ImportedBam, bcs::ImportedBcs, image::ImportedImage,
            tis::ImportedTis,
        };
        use infinitier_bam_resource::BamImporter;
        use infinitier_bcs_resource::BcsImporter;
        use infinitier_bmp_resource::BmpImporter;
        use infinitier_cre_resource::CreImporter;
        use infinitier_fnt_resource::FntImporter;
        use infinitier_gam_resource::GamImporter;
        use infinitier_ids_resource::IdsImporter;
        use infinitier_ini_resource::IniImporter;
        use infinitier_itm_resource::ItmImporter;
        use infinitier_mos_resource::MosImporter;
        use infinitier_png_resource::PngImporter;
        use infinitier_pvrz_resource::PvrzImporter;
        use infinitier_sav_resource::SavImporter;
        use infinitier_spl_resource::SplImporter;
        use infinitier_tis_resource::TisImporter;
        use infinitier_tlk_resource::TlkImporter;
        use infinitier_ttf_resource::TtfImporter;
        use infinitier_two_da_resource::TwoDAImporter;
        use infinitier_wed_resource::WedImporter;

        let ds = self
            .datasource
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no datasource available"))?;
        let imported = match self.r#type {
            ResourceType::Acm => AcmImporter { name: &self.name }
                .import(ds)
                .map(SoundDecoder::from)
                .map(ImportedResource::Sound),
            ResourceType::Are => Ok(ImportedResource::Are),
            ResourceType::Bah => Ok(ImportedResource::Bah),
            ResourceType::Bam => BamImporter { name: &self.name }
                .import(ds)
                .and_then(|bam| ImportedBam::load(bam, game_data))
                .map(ImportedResource::Bam),
            ResourceType::Bcs | ResourceType::Bs => BcsImporter { name: &self.name }
                .import(ds)
                .and_then(|bcs| ImportedBcs::load(bcs, game_data))
                .map(ImportedResource::Bcs),
            ResourceType::Bio => Ok(ImportedResource::Bio),
            ResourceType::Bmp => BmpImporter { name: &self.name }
                .import(ds)
                .map(ImportedImage::from_bmp)
                .map(ImportedResource::Image),
            ResourceType::Chr => Ok(ImportedResource::Chr),
            ResourceType::Chu => Ok(ImportedResource::Chu),
            ResourceType::Cre => CreImporter {
                name: &self.name,
                game: game_data.game(),
            }
            .import(ds)
            .map(|cre| ImportedResource::Cre(Box::new(cre))),
            ResourceType::Dlg => Ok(ImportedResource::Dlg),
            ResourceType::Eff => Ok(ImportedResource::Eff),
            ResourceType::Fnt => FntImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Fnt),
            ResourceType::Gam => GamImporter {
                name: &self.name,
                engine: game_data.game().engine(),
            }
            .import(ds)
            .and_then(|gam| crate::imported_resource::gam::ImportedGam::load(gam, game_data))
            .map(|imported| ImportedResource::Gam(Box::new(imported))),
            ResourceType::Glsl => Ok(ImportedResource::Glsl),
            ResourceType::Gui => Ok(ImportedResource::Gui),
            ResourceType::Ids => IdsImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ids),
            ResourceType::Ini => IniImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ini),
            ResourceType::Itm => ItmImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Itm),
            ResourceType::Lua => Ok(ImportedResource::Lua),
            ResourceType::Maze => Ok(ImportedResource::Maze),
            ResourceType::Menu => Ok(ImportedResource::Menu),
            ResourceType::Mos => MosImporter { name: &self.name }
                .import(ds)
                .and_then(|mos| ImportedImage::from_mos(mos, game_data))
                .map(ImportedResource::Image),
            ResourceType::Mus => Ok(ImportedResource::Mus),
            ResourceType::Mve => Ok(ImportedResource::Mve(movie::MovieSource::new(
                ds.clone(),
                &self.name,
            ))),
            ResourceType::Plt => Ok(ImportedResource::Plt),
            ResourceType::Png => PngImporter { name: &self.name }
                .import(ds)
                .map(ImportedImage::from_png)
                .map(ImportedResource::Image),
            ResourceType::Pro => Ok(ImportedResource::Pro),
            ResourceType::Pvrz => PvrzImporter { name: &self.name }
                .import(ds)
                .map(ImportedImage::from_pvrz)
                .map(ImportedResource::Image),
            ResourceType::Sav => SavImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Sav),
            ResourceType::Spl => SplImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Spl),
            ResourceType::Sql => Ok(ImportedResource::Sql),
            ResourceType::Src => Ok(ImportedResource::Src),
            ResourceType::Sto => Ok(ImportedResource::Sto),
            ResourceType::Tga => Ok(ImportedResource::Tga),
            ResourceType::Tis => TisImporter { name: &self.name }
                .import(ds)
                .and_then(|tis| ImportedTis::load(tis, game_data, &self.name))
                .map(ImportedResource::Tis),
            ResourceType::Tlk => {
                let source = ds
                    .clone()
                    .with_encoding(dialog_tlk_encoding(self.game_type));
                TlkImporter { name: &self.name }
                    .import(&source)
                    .map(ImportedResource::Tlk)
            }
            ResourceType::Toh => Ok(ImportedResource::Toh),
            ResourceType::Tot => Ok(ImportedResource::Tot),
            ResourceType::Ttf => TtfImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Ttf),
            ResourceType::TwoDA => TwoDAImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::TwoDA),
            ResourceType::Unknown(id) => Ok(ImportedResource::Unknown(id)),
            ResourceType::Vef => Ok(ImportedResource::Vef),
            ResourceType::Vvc => Ok(ImportedResource::Vvc),
            ResourceType::Wav => Ok(WavImporter { name: &self.name }
                .import(ds)
                .map(SoundDecoder::Wav)
                .map(ImportedResource::Sound)?),
            ResourceType::Wbm => Ok(ImportedResource::Wbm(movie::MovieSource::new(
                ds.clone(),
                &self.name,
            ))),
            ResourceType::Wed => WedImporter { name: &self.name }
                .import(ds)
                .map(ImportedResource::Wed),
            ResourceType::Wfx => Ok(ImportedResource::Wfx),
            ResourceType::Wmp => Ok(ImportedResource::Wmp),
        }?;
        Ok(Cow::Owned(imported))
    }
}

/// A game data builder
pub struct GameDataBuilder {
    /// File system
    fs: CaseInsensitiveFS,
    /// Name of the key file
    key_file: String,
    /// Resource overrides folders
    overrides: Vec<String>,
    /// Resource types to preload
    preload: Vec<ResourceType>,
    /// Game Type
    game_type: Option<Game>,
}

impl GameDataBuilder {
    /// Create a new game data builder.
    /// If the game type is not provided, the game type will be inferred from the game root.
    pub fn new(game_root: impl Roots, game_type: Option<Game>) -> io::Result<GameDataBuilder> {
        Ok(GameDataBuilder {
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
            preload: vec![
                ResourceType::TwoDA,
                ResourceType::Ids,
                ResourceType::Spl,
                ResourceType::Itm,
                ResourceType::Tlk,
            ],
            key_file: "chitin.key".to_string(),
        })
    }

    /// Set the key file name
    /// Default: "chitin.key"
    pub fn with_key_file(mut self, key_file: String) -> GameDataBuilder {
        self.key_file = key_file;
        self
    }

    /// Set the resource override folders
    /// Default: ["override"]
    pub fn with_overrides(mut self, overrides: Vec<String>) -> GameDataBuilder {
        self.overrides = overrides;
        self
    }

    /// Set the resource types to preload.
    /// Default: `[2DA, IDS, SPL, ITM]`. Pass an empty `Vec` to disable
    /// preloading entirely.
    pub fn with_preload(mut self, preload: Vec<ResourceType>) -> GameDataBuilder {
        self.preload = preload;
        self
    }

    /// Build the game data
    pub fn build(&self) -> io::Result<GameData> {
        debug!("Building game data");

        let key_path = self.fs.get_path(&self.key_file)?;
        let key = KeyImporter {
            name: &self.key_file,
        }
        .import(&DataSource::new(key_path.path()))?;

        let game_type = if let Some(game_type) = self.game_type {
            game_type
        } else {
            detect_game(&self.fs, Some(&key)).ok_or_else(|| {
                error!("Failed to detect game type");
                Error::other("Failed to detect game type")
            })?
        };

        let mut game_data = GameData {
            fs: self.fs.clone(),
            game_type,
            resources: vec![],
            name_type_index: HashMap::new(),
            type_index: HashMap::new(),
        };

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
                            count,
                            offset,
                            r#type: _,
                        } => {
                            // BIF Tileset entries carry only the raw tile
                            // data (no `"TIS V1  "` signature). Reconstruct
                            // the standalone TIS shape lazily by
                            // concatenating an in-memory header with the
                            // tile bytes still living inside the BIF —
                            // `DataSource::Concat` reads its parts on
                            // demand, so the tile bytes are not pulled
                            // into memory.
                            let tile_total = (*count as u64) * (*size as u64);
                            let header = synthesise_tis_header(*count, *size);
                            let header_len = header.len() as u64;
                            let header_ds = DataSource::new(header);
                            let tile_ds = bif_source(*offset, tile_total);
                            let concat = DataSource::new_concat(vec![header_ds, tile_ds]);
                            (concat, header_len + tile_total)
                        }
                    };

                    game_data.add_resource(GameResource {
                        game_type,
                        name,
                        r#type,
                        file_size: Some(file_size),
                        datasource: Some(datasource),
                        data_origin: DataOrigin::Bif {
                            name: bif.name.clone(),
                        },
                        imported: None,
                    });
                } else {
                    warn!(
                        "Resource {}.{:?} not found in bif {:?}",
                        name, r#type, bif_ds
                    );
                    game_data.add_resource(GameResource {
                        game_type,
                        name,
                        r#type,
                        file_size: None,
                        datasource: None,
                        data_origin: DataOrigin::Missing,
                        imported: None,
                    });
                }
            } else {
                warn!("Resource {}.{:?} not found", name, r#type);
                game_data.add_resource(GameResource {
                    game_type,
                    name,
                    r#type,
                    file_size: None,
                    datasource: None,
                    data_origin: DataOrigin::Missing,
                    imported: None,
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

        let tlk = self.add_resources_from_dir(
            &mut game_data,
            "",
            ResourceType::Tlk.get_extension(),
            false,
        )?;

        if tlk == 0 {
            // Workaround for no tlk found in root directory
            self.add_resources_from_dir(
                &mut game_data,
                "lang/en_us",
                ResourceType::Tlk.get_extension(),
                false,
            )?;
        }

        self.add_resources_from_dir(&mut game_data, "override", None, false)?;

        self.preload_resources(&mut game_data);

        Ok(game_data)
    }

    /// Import every resource whose type is in [`self.preload`](Self::preload)
    /// and cache the result in [`GameResource::imported`], so later lookups
    /// return the value borrowed instead of re-importing.
    ///
    /// Runs after all resources are registered (importers may resolve
    /// cross-references against the fully-built `game_data`). Import failures
    /// are logged and skipped — a resource that fails to preload simply stays
    /// un-cached and imports lazily on first access.
    fn preload_resources(&self, game_data: &mut GameData) {
        if self.preload.is_empty() {
            return;
        }

        // First pass: import under a shared borrow of `game_data` (importers
        // need it to resolve references). Collect owned results keyed by id so
        // the borrow ends before we mutate.
        let mut preloaded: Vec<(ResourceId, ImportedResource)> = Vec::new();
        for (id, resource) in game_data.resources.iter().enumerate() {
            if resource.datasource.is_none() || !self.preload.contains(&resource.r#type) {
                continue;
            }
            match resource.import(game_data) {
                Ok(imported) => preloaded.push((id, imported.into_owned())),
                Err(e) => warn!(
                    "Failed to preload {}.{:?}: {e}",
                    resource.name, resource.r#type
                ),
            }
        }

        // Second pass: store the cached imports.
        for (id, imported) in preloaded {
            game_data.resources[id].imported = Some(imported);
        }
    }

    fn add_resources_from_dir(
        &self,
        game: &mut GameData,
        dir_name: &str,
        extension: Option<&str>,
        recursive: bool,
    ) -> io::Result<usize> {
        let mut count = 0; // number of resources found 
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
                game_type: game.game_type,
                r#type,
                name,
                imported: None,
            });
            count += 1;
        }
        Ok(count)
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
        GameDataBuilder::new(bg_root, Some(Game::Bg2))
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
        WedImporter { name: "ar0714" }
            .import(resource.datasource.as_ref().unwrap())
            .unwrap();
    }

    #[test]
    fn test_tis_resource_found() {
        use std::io::Read;
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

        // BIF-embedded TIS datasources are a `Concat` of a synthesised
        // 24-byte header + the raw tile bytes still living inside the
        // BIF — the first 8 bytes of the resource must therefore be
        // the `"TIS V1  "` signature.
        let ds = resource.datasource.as_ref().expect("datasource present");
        let mut reader = ds.reader().unwrap();
        let mut sig = [0u8; 8];
        reader.read_exact(&mut sig).unwrap();
        assert_eq!(&sig, b"TIS V1  ");

        // file_size is reported as the full standalone-TIS-equivalent
        // payload (header + tile bytes), so it lines up with what the
        // importer reads.
        let total = ds.len().unwrap();
        assert_eq!(resource.file_size, Some(total));
        assert!(total >= 24);
    }

    #[test]
    fn test_tis_bif_resource_imports_end_to_end() {
        // Regression: the explorer used to crash with "Unsupported TIS
        // file signature" because BIF tilesets are header-less. The
        // BIF-loader now glues a synthesised header onto the raw tile
        // bytes via `DataSource::new_concat`, so importing must yield a
        // real `ImportedTis` with all tiles decoded to 64×64 RGBA.
        use crate::imported_resource::ImportedResource;

        let game_data = build_bg2();
        let resource = game_data
            .get_by_name_and_type("ar0714", ResourceType::Tis)
            .unwrap();

        let imported = resource
            .import(&game_data)
            .expect("TIS import succeeds")
            .into_owned();
        let ImportedResource::Tis(tis) = imported else {
            panic!("expected ImportedResource::Tis");
        };
        assert!(tis.tile_count > 0);
        assert_eq!(tis.tile_dimension, 64);
        // Eagerly-decoded RGBA tiles: tile_count × 64 × 64 × 4 bytes.
        assert_eq!(
            tis.tile_pixels.len(),
            tis.tile_count * 64 * 64 * 4,
            "decoded tile pixel buffer size"
        );
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
            imported: None,
        }
    }

    #[test]
    fn test_add_resource_replaces_when_name_and_type_match() {
        let mut game_data =
            GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());

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
        let mut game_data =
            GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());

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

        let builder = GameDataBuilder::new(root, Some(Game::Bg2)).unwrap();
        let mut game_data =
            GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());

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

        let builder = GameDataBuilder::new(root, Some(Game::Bg2)).unwrap();
        let mut game_data =
            GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());

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
        let mut game_data =
            GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());

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
        let mut game_data =
            GameData::new(vec![], Game::Bg2, infinitier_fs::CaseInsensitiveFS::empty());
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
            .get_all_resources_by_type(ResourceType::Bam)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(bam_names, vec!["A", "C"]);

        let wed_names: Vec<&str> = game_data
            .get_all_resources_by_type(ResourceType::Wed)
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(wed_names, vec!["B"]);

        // The replacement must be observable through the type index.
        let a = game_data
            .get_all_resources_by_type(ResourceType::Bam)
            .find(|r| r.name == "A")
            .unwrap();
        assert_eq!(
            a.data_origin,
            DataOrigin::Bif {
                name: "data/replacement.bif".to_string(),
            }
        );

        // Unknown type → empty.
        assert_eq!(
            game_data
                .get_all_resources_by_type(ResourceType::Tga)
                .count(),
            0
        );
    }

    // ── dialog.tlk encoding ──────────────────────────────────────────

    /// Build a minimal one-entry TLK V1 byte stream whose single string
    /// is `string_bytes` (raw, undecoded).
    fn synth_tlk_bytes(string_bytes: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"TLK ");
        buf.extend_from_slice(b"V1  ");
        buf.extend_from_slice(&0u16.to_le_bytes()); // language id
        buf.extend_from_slice(&1u32.to_le_bytes()); // entry count
        let strings_offset: u32 = 18 + 26; // header + one entry
        buf.extend_from_slice(&strings_offset.to_le_bytes());
        // Entry 0: flags, sound resref, variances, then (offset, length)
        // into the strings section.
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&[0u8; 8]);
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&(string_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(string_bytes);
        buf
    }

    /// A `GameData` carrying a single in-memory `dialog.tlk`, tagged as
    /// `game` so the importer picks the matching encoding.
    fn game_data_with_dialog(game: Game, tlk_bytes: Vec<u8>) -> GameData {
        let res = GameResource {
            game_type: game,
            name: "dialog".to_string(),
            r#type: ResourceType::Tlk,
            file_size: Some(tlk_bytes.len() as u64),
            datasource: Some(DataSource::new(tlk_bytes)),
            data_origin: DataOrigin::Missing,
            imported: None,
        };
        GameData::new(vec![res], game, infinitier_fs::CaseInsensitiveFS::empty())
    }

    #[test]
    fn dialog_tlk_encoding_is_utf8_for_ee_and_w1252_for_classic() {
        for ee in [
            Game::Bgee,
            Game::BgeeSod,
            Game::Bg2ee,
            Game::Eet,
            Game::Iwdee,
            Game::Pstee,
        ] {
            assert_eq!(dialog_tlk_encoding(ee), encoding_rs::UTF_8, "{ee:?}");
        }
        for classic in [
            Game::Bg,
            Game::Bg2,
            Game::Tutu,
            Game::Iwd {
                heart_of_winter: false,
                totl: false,
            },
            Game::Iwd2,
            Game::Pst,
        ] {
            assert_eq!(
                dialog_tlk_encoding(classic),
                encoding_rs::WINDOWS_1252,
                "{classic:?}"
            );
        }
    }

    /// Enhanced Edition `dialog.tlk` is UTF-8: a multi-byte glyph (the
    /// "ō" in "Ninjatō", UTF-8 `0xC5 0x8D`) must decode intact rather
    /// than mojibake into "Å…" as it would under WINDOWS-1252.
    /// Regression for the Proficiencies-tab "Scimitar / Wakizashi /
    /// Ninjatō" label.
    #[test]
    fn ee_dialog_tlk_decodes_utf8_multibyte_glyphs() {
        let bytes = synth_tlk_bytes("Ninjatō".as_bytes());

        let ee = game_data_with_dialog(Game::Bgee, bytes.clone());
        assert_eq!(ee.dialog_tlk().unwrap().get(0).as_deref(), Some("Ninjatō"));

        // The very same bytes under a classic game are decoded as
        // WINDOWS-1252 → not equal, proving the per-game selection
        // actually changes the result.
        let classic = game_data_with_dialog(Game::Bg, bytes);
        assert_ne!(
            classic.dialog_tlk().unwrap().get(0).as_deref(),
            Some("Ninjatō")
        );
    }
}
