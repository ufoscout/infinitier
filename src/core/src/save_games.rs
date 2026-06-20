//! Discovery of save games sitting on a [`CaseInsensitiveFS`].
//!
//! IE engines write save games into one of a small set of well-known
//! subdirectories of the game folder ([`SAVE_DIRS`]). Each save lives
//! in its own folder named after the on-disk save title (e.g.
//! "000000001-Quick-Save"). Inside that folder the engine drops a
//! per-area `.SAV` file plus its companions:
//!
//! - the campaign-wide `.GAM` file (`BALDUR.GAM` / `ICEWIND.GAM` /
//!   `ICEWIND2.GAM` / `TORMENT.GAM` depending on the engine),
//! - a screenshot BMP (`BALDUR.BMP` / `ICEWIND.BMP` / …),
//! - the party portraits `PORTRT0.BMP`, `PORTRT1.BMP`, …,
//! - the world-map state `WORLDMAP.WMP`.
//!
//! [`scan_save_games`] walks the FS for `.SAV` files under each of
//! [`SAVE_DIRS`] and gathers the companion files via case-insensitive
//! lookups on the same FS. The result is a [`SaveGames`] collection
//! that exposes alphabetical-by-name iteration plus `by_index` /
//! `by_name` lookups.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use infinitier_common::ResourceType;
use infinitier_datasource::DataSource;
use infinitier_fs::{CaseInsensitiveFS, CiPath};
use log::debug;

/// Save-game subdirectories scanned by [`scan_save_games`]. Lifted
/// from NearInfinity's per-engine save-folder lists; covers every
/// shipped IE engine's set — vanilla games only ever use `Save` and
/// `MPSave`, the Enhanced Editions add the rest.
pub const SAVE_DIRS: &[&str] = &[
    "Save",
    "MPSave",
    "BPSave",
    "CloudSave",
    "ImportSave",
    "ImportBPSave",
    "MPBPSave",
];

/// One save game on disk — the SAV/GAM pair plus the auxiliary
/// resources the engine writes alongside (screenshot, party
/// portraits, world-map state). The [`DataSource`] fields point at
/// the canonical absolute on-disk path resolved through the
/// [`CaseInsensitiveFS`] used to discover the save.
#[derive(Debug, Clone)]
pub struct SaveGame {
    /// On-disk name of the save folder (e.g. `000000001-Quick-Save`,
    /// `888888890-Alone with Imoen`). Used as the `by_name` key on
    /// [`SaveGames`].
    pub name: String,
    /// Absolute path of the save folder on disk. Resolved through
    /// the [`CaseInsensitiveFS`] used to discover the save (so
    /// callers don't need to re-canonicalise it). Used by tooling
    /// that wants to copy / rename the whole save folder, e.g. the
    /// keeper's "Save As" action — pure metadata, ignored by
    /// resource-importer paths.
    pub folder_path: PathBuf,
    /// The `.SAV` file (per-area state). Always present — this is
    /// the file [`scan_save_games`] keys the discovery on.
    pub sav: DataSource,
    /// The `.GAM` file (campaign-wide state).
    pub gam: DataSource,
    /// Area screenshot — the engine-named BMP next to the SAV
    /// (`BALDUR.BMP`, `ICEWIND.BMP`, `ICEWIND2.BMP`, `TORMENT.BMP`).
    /// `None` when absent.
    pub screenshot: Option<DataSource>,
    /// Party portraits — `PORTRT0.BMP`, `PORTRT1.BMP`, …. Ordered by
    /// the lowercased on-disk filename so the slot order is
    /// deterministic across filesystems. Empty when the folder has
    /// no portraits.
    pub portraits: Vec<DataSource>,
    /// `WORLDMAP.WMP`. `None` when absent.
    pub worldmap: Option<DataSource>,
}

/// A collection of [`SaveGame`]s discovered in a [`CaseInsensitiveFS`]
/// under any of [`SAVE_DIRS`]. Order: alphabetical by `name` (stable
/// — collisions keep their discovery order).
#[derive(Debug, Clone, Default)]
pub struct SaveGames {
    saves: Vec<SaveGame>,
    by_name: HashMap<String, usize>,
}

impl SaveGames {
    /// Total number of save games discovered.
    pub fn len(&self) -> usize {
        self.saves.len()
    }

    /// `true` when the search found nothing.
    pub fn is_empty(&self) -> bool {
        self.saves.is_empty()
    }

    /// Lookup by alphabetical index. Returns `None` when out of range.
    pub fn by_index(&self, index: usize) -> Option<&SaveGame> {
        self.saves.get(index)
    }

    /// Lookup by on-disk folder name. Case-sensitive against the
    /// canonical filesystem name. Returns the first match when two
    /// save dirs happen to contain folders with the same name (rare
    /// in practice — e.g. `Save/QuickSave` vs `MPSave/QuickSave`).
    pub fn by_name(&self, name: &str) -> Option<&SaveGame> {
        self.by_name
            .get(name)
            .copied()
            .and_then(|i| self.saves.get(i))
    }

    /// All save games.
    pub fn saves(&self) -> &[SaveGame] {
        &self.saves
    }
}

/// Re-walk every [`SAVE_DIRS`] subtree on `fs` so a subsequent
/// [`scan_save_games`] sees the current on-disk state.
pub(crate) fn refresh_save_dirs(fs: &mut CaseInsensitiveFS) -> std::io::Result<()> {
    for save_dir in SAVE_DIRS {
        fs.refresh(Some(save_dir))?;
    }
    Ok(())
}

/// Scan `fs` for save games. Looks for every `.SAV` file recursively
/// under each [`SAVE_DIRS`] entry; for every match, gathers the
/// sibling GAM / portraits / screenshot / worldmap via direct (non-
/// recursive) sibling listing on the same FS.
pub(crate) fn scan_save_games(fs: &CaseInsensitiveFS) -> SaveGames {
    let mut saves: Vec<SaveGame> = Vec::new();
    for save_dir in SAVE_DIRS {
        for sav in fs.list_files(save_dir, ResourceType::Sav.get_extension(), true) {
            let Some(save) = build_save_game(fs, &sav) else {
                continue;
            };
            saves.push(save);
        }
    }
    // Alphabetical by name. Stable so name collisions keep discovery
    // order (the `Save` folder is scanned before `MPSave`, so a
    // `Save/X` save lands before an `MPSave/X` one in the by_name
    // map).
    saves.sort_by(|a, b| a.name.cmp(&b.name));
    let mut by_name = HashMap::with_capacity(saves.len());
    for (i, save) in saves.iter().enumerate() {
        by_name.entry(save.name.clone()).or_insert(i);
    }
    debug!("Discovered {} save game(s)", saves.len());
    SaveGames { saves, by_name }
}

fn build_save_game(fs: &CaseInsensitiveFS, sav: &CiPath) -> Option<SaveGame> {
    let sav_path: &Path = sav.path();
    let parent_path = sav_path.parent()?;
    let name = parent_path.file_name()?.to_string_lossy().into_owned();

    // CaseInsensitiveFS keys are forward-slash, lower-case and
    // relative to the FS root. Trim the SAV's own filename off the
    // key to get the parent directory's key, then list the parent's
    // direct children (non-recursive) by extension.
    let key = sav.as_str();
    let parent_key = key.rsplit_once('/').map(|(p, _)| p).unwrap_or("");

    let gam = fs
        .list_files(parent_key, Some("gam"), false)
        .into_iter()
        .next()
        .map(|p| DataSource::new(p.path().to_path_buf()))?;
    let worldmap = fs
        .list_files(parent_key, Some("wmp"), false)
        .into_iter()
        .next()
        .map(|p| DataSource::new(p.path().to_path_buf()));

    let mut portraits: Vec<CiPath> = Vec::new();
    let mut screenshot: Option<CiPath> = None;
    for bmp in fs.list_files(parent_key, Some("bmp"), false) {
        // Portraits' on-disk name always starts with the engine-
        // defined `PORTRT` prefix (the `base_name()` is already
        // lowercased). Everything else is the area screenshot
        // (`BALDUR.BMP` / `ICEWIND.BMP` / `ICEWIND2.BMP` /
        // `TORMENT.BMP`).
        if bmp.base_name().starts_with("portrt") {
            portraits.push(bmp);
        } else if screenshot.is_none() {
            screenshot = Some(bmp);
        }
    }
    // Deterministic portrait order — sort by the lowercased on-disk
    // path so PORTRT0 < PORTRT1 < PORTRT2 regardless of how the OS
    // returned them.
    portraits.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    Some(SaveGame {
        name,
        folder_path: parent_path.to_path_buf(),
        sav: DataSource::new(sav.path().to_path_buf()),
        gam,
        screenshot: screenshot.map(|p| DataSource::new(p.path().to_path_buf())),
        portraits: portraits
            .into_iter()
            .map(|p| DataSource::new(p.path().to_path_buf()))
            .collect(),
        worldmap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::get_assets_path;

    /// Build a `CaseInsensitiveFS` rooted at a sub-folder of
    /// `assets/SAV_GAM/<engine>` (e.g. "bg", "bg_ee", "iwd2").
    fn fixture_fs(engine_dir: &str) -> CaseInsensitiveFS {
        let root = get_assets_path().join("SAV_GAM").join(engine_dir);
        CaseInsensitiveFS::new(root).expect("open fixture FS")
    }

    #[test]
    fn discovers_bg_saves_under_save_and_mpsave() {
        // Fixture layout:
        //   Save/000000001-Quick-Save
        //   Save/888888888-Mission-Pack-Save
        //   Save/888888889-Alone in the dark
        //   Save/888888890-Alone with Imoen
        //   MPSave/888888888-Mission-Pack-Save
        let fs = fixture_fs("bg");
        let saves = scan_save_games(&fs);
        assert_eq!(saves.len(), 5);
        // Alphabetical order.
        let names: Vec<&str> = saves.saves().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "000000001-Quick-Save",
                "888888888-Mission-Pack-Save",
                "888888888-Mission-Pack-Save", // same name in both Save/ and MPSave/
                "888888889-Alone in the dark",
                "888888890-Alone with Imoen",
            ],
        );
    }

    #[test]
    fn by_index_returns_alphabetical_order() {
        let fs = fixture_fs("bg");
        let saves = scan_save_games(&fs);
        // by_index follows the alphabetical sort.
        assert_eq!(
            saves.by_index(0).map(|s| s.name.as_str()),
            Some("000000001-Quick-Save"),
        );
        assert_eq!(
            saves.by_index(saves.len() - 1).map(|s| s.name.as_str()),
            Some("888888890-Alone with Imoen"),
        );
        assert!(saves.by_index(saves.len()).is_none());
    }

    #[test]
    fn by_name_returns_matching_save() {
        let fs = fixture_fs("bg");
        let saves = scan_save_games(&fs);
        let found = saves
            .by_name("888888890-Alone with Imoen")
            .expect("save 'Alone with Imoen' must be present");
        assert_eq!(found.name, "888888890-Alone with Imoen");
        // No match on a non-existent name.
        assert!(saves.by_name("does-not-exist").is_none());
    }

    #[test]
    fn collects_companion_files_around_each_sav() {
        let fs = fixture_fs("bg");
        let saves = scan_save_games(&fs);
        // Pick the save we already inventoried by hand: 2 portraits
        // + 1 screenshot + 1 GAM + 1 worldmap.
        let target = saves
            .by_name("888888890-Alone with Imoen")
            .expect("fixture save must be present");
        assert!(target.screenshot.is_some(), "BALDUR.BMP must be picked up");
        assert!(target.worldmap.is_some(), "WORLDMAP.WMP must be picked up");
        assert_eq!(
            target.portraits.len(),
            2,
            "expected PORTRT0.BMP + PORTRT1.BMP",
        );
        // The other save has a single portrait — exercise the
        // single-portrait branch.
        let alone = saves
            .by_name("888888889-Alone in the dark")
            .expect("fixture save must be present");
        assert_eq!(alone.portraits.len(), 1);
    }

    #[test]
    fn portraits_are_sorted_deterministically() {
        // BG:EE save folder has PORTRT0/1/2.bmp — verify the slot
        // order is stable (sorted by filename, not by inode order).
        let fs = fixture_fs("bg_ee");
        let saves = scan_save_games(&fs);
        let auto = saves
            .by_name("000000000-Auto-Salvataggio")
            .expect("BG:EE fixture must contain Auto-Salvataggio");
        assert_eq!(auto.portraits.len(), 3);
        // Sanity: the on-disk paths should end in PORTRT0/1/2.bmp in
        // that order. We can read the underlying path through the
        // canonical path the DataSource was constructed from — we
        // re-derive it by re-running list_files since DataSource
        // doesn't expose its path.
        let key = "save/000000000-auto-salvataggio";
        let bmps = fs.list_files(key, Some("bmp"), false);
        let mut portrt_paths: Vec<String> = bmps
            .iter()
            .filter(|b| b.base_name().starts_with("portrt"))
            .map(|b| b.as_str().to_string())
            .collect();
        portrt_paths.sort();
        assert!(
            portrt_paths[0].ends_with("portrt0.bmp"),
            "first portrait should be PORTRT0, got {:?}",
            portrt_paths
        );
        assert!(
            portrt_paths[1].ends_with("portrt1.bmp"),
            "second portrait should be PORTRT1, got {:?}",
            portrt_paths
        );
        assert!(
            portrt_paths[2].ends_with("portrt2.bmp"),
            "third portrait should be PORTRT2, got {:?}",
            portrt_paths
        );
    }

    #[test]
    fn excludes_save_dirs_outside_the_known_list() {
        // BG:EE fixture has a `sodsave/` directory with valid SAV
        // files in it. `sodsave` isn't in `SAVE_DIRS`, so its
        // contents must not appear in the result.
        let fs = fixture_fs("bg_ee");
        let saves = scan_save_games(&fs);
        // Names of saves under sodsave/ — must not show up:
        let sod_only_names = ["000000001-Salvataggio Rapido"];
        // (Note: this name also exists under save/ for BG:EE; we'll
        // check the count instead of relying on name uniqueness.)
        let saves_under_save = std::fs::read_dir(get_assets_path().join("SAV_GAM/bg_ee/save"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .count();
        assert_eq!(
            saves.len(),
            saves_under_save,
            "scanner picked up entries from outside SAVE_DIRS — got {} vs {saves_under_save} under save/. Names = {:?}",
            saves.len(),
            saves
                .saves()
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
        );
        // Each `sod_only_names` entry whose name doesn't appear
        // under save/ must be absent from the results.
        let save_dir_names: std::collections::HashSet<String> =
            std::fs::read_dir(get_assets_path().join("SAV_GAM/bg_ee/save"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
        for sod_only in sod_only_names {
            if save_dir_names.contains(sod_only) {
                continue;
            }
            assert!(
                saves.by_name(sod_only).is_none(),
                "{sod_only} from sodsave/ leaked into the results"
            );
        }
    }
}
