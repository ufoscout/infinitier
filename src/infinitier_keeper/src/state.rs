//! Loaded keeper state. Owned by the root `KeeperApp` view; mutated
//! through `cx.listener` closures when the user clicks a party row or
//! a tab chip. Mirrors the Slint spike's `AppState` one-for-one.

use std::path::PathBuf;

use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

pub struct KeeperState {
    pub game_data: GameData,
    pub save_name: String,
    /// Absolute on-disk path of the save folder the keeper opened.
    /// Used by the Save action to compute the destination folder
    /// (sibling of this one) and to enumerate the files to copy.
    pub save_folder_path: PathBuf,
    pub imported_gam: ImportedGam,
    /// Cap ranges + 2DA-driven bonus tables for the active engine.
    /// Built once at startup via [`EngineCaps::new`] and consulted
    /// by the abilities tab for clamping and live bonus display.
    pub engine_caps: EngineCaps,
}
