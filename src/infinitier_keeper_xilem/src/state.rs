//! App-wide state — the same shape as the egui keeper's `AppState`,
//! framework-agnostic (only `infinitier_core` + the theme). The Xilem
//! `app_logic` takes `&mut AppState` and rebuilds the view from it.

use std::path::PathBuf;

use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

use crate::tabs::CharacterTab;

/// Top-level keeper state.
pub struct AppState {
    pub game_data: GameData,
    #[allow(dead_code)] // kept for parity / future effective-bonus rows
    pub engine_caps: EngineCaps,
    pub tabs: Vec<SaveTab>,
    pub active_tab: usize,
    /// Dark-mode flag. `app_logic` derives the [`xilem_components::Theme`]
    /// from this each pass, so toggling the theme is just flipping a bool.
    pub dark: bool,
}

/// Per-save state for one open save game.
pub struct SaveTab {
    pub save_name: String,
    #[allow(dead_code)] // kept for parity with the egui keeper's save action
    pub save_folder_path: PathBuf,
    pub save: Box<ImportedGam>,
    pub selected_party_index: Option<usize>,
    pub selected_tab: CharacterTab,
}

impl AppState {
    pub fn window_title(&self) -> String {
        format!("Infinitier Keeper (Xilem) - {:?}", self.game_data.game())
    }

    pub fn new(
        game_data: GameData,
        save_name: String,
        save_folder_path: PathBuf,
        save: Box<ImportedGam>,
        engine_caps: EngineCaps,
    ) -> Self {
        let tab = SaveTab::new(save_name, save_folder_path, save);
        Self {
            game_data,
            engine_caps,
            tabs: vec![tab],
            active_tab: 0,
            dark: false,
        }
    }

    pub fn active(&self) -> &SaveTab {
        &self.tabs[self.active_tab]
    }

    pub fn active_mut(&mut self) -> &mut SaveTab {
        &mut self.tabs[self.active_tab]
    }
}

impl SaveTab {
    pub fn new(save_name: String, save_folder_path: PathBuf, save: Box<ImportedGam>) -> Self {
        let selected_party_index = if save.party_npcs.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            save_name,
            save_folder_path,
            save,
            selected_party_index,
            selected_tab: CharacterTab::Abilities,
        }
    }
}
