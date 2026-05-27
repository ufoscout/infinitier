//! Loaded keeper state. Owned by the root `KeeperApp` view; mutated
//! through `cx.listener` closures when the user clicks a party row or
//! a tab chip. Mirrors the Slint spike's `AppState` one-for-one.

use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

pub struct KeeperState {
    pub game_data: GameData,
    pub save_name: String,
    pub imported_gam: ImportedGam,
}
