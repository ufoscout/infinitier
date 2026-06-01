//! App-wide state — the same shape as the egui keeper's `AppState`,
//! framework-agnostic (only `infinitier_core` + the theme). The Xilem
//! `app_logic` takes `&mut AppState` and rebuilds the view from it.

use std::collections::HashMap;
use std::path::PathBuf;

use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

use crate::fields::EditableField;
use crate::tabs::CharacterTab;

/// Top-level keeper state.
pub struct AppState {
    pub game_data: GameData,
    /// Cap ranges + 2DA bonus tables; consulted when clamping edits.
    pub engine_caps: EngineCaps,
    pub tabs: Vec<SaveTab>,
    pub active_tab: usize,
    /// Dark-mode flag. `app_logic` derives the [`xilem_components::Theme`]
    /// from this each pass, so toggling the theme is just flipping a bool.
    pub dark: bool,
    /// In-flight text per editable abilities-tab field. Refreshed from
    /// the active CRE/GAM whenever the bound `(save tab, party slot)`
    /// changes; each text input writes its raw string here on every
    /// keystroke and commits (parse + clamp + write-back) on Enter.
    pub editors: HashMap<EditableField, String>,
    /// The `(active_tab, selected_party_index)` the `editors` buffers were
    /// last filled for. `None` forces a refresh on the next pass.
    pub editors_bound_to: Option<(usize, Option<usize>)>,
    /// Last Save outcome, surfaced in the header bar.
    pub status: Option<String>,
}

/// Per-save state for one open save game.
pub struct SaveTab {
    pub save_name: String,
    /// Absolute path of the open save folder; the Save action writes a
    /// sibling `<name> (Edited NNNN)` folder next to it.
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
            editors: HashMap::new(),
            editors_bound_to: None,
            status: None,
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
