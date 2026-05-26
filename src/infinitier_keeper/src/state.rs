//! App-wide mutable state, separated from the egui scaffolding so
//! the UI panels can borrow it without coupling to the eframe shell.

use infinitier_core::game::GameData;

use crate::save::SaveGame;
use crate::ui::CharacterTab;

/// Top-level keeper state. Loaded once at startup; refreshed on
/// reload (future work — for now the loader runs in `main`).
pub struct AppState {
    /// Pre-indexed game data. Currently unused for the MVP party
    /// view (the CRE blob is self-contained inside the save), but
    /// kept here so subsequent features (item lookup, spell names,
    /// 2DA references) can read from it without re-plumbing.
    pub game_data: GameData,
    /// Loaded save state.
    pub save: SaveGame,
    /// Selected party-member row, or `None` until the user clicks
    /// one.
    pub selected_party_index: Option<usize>,
    /// Currently-active tab in the per-character editor on the right.
    pub selected_tab: CharacterTab,
}

impl AppState {
    pub fn new(game_data: GameData, save: SaveGame) -> Self {
        let selected_party_index = if save.party.is_empty() { None } else { Some(0) };
        Self {
            game_data,
            save,
            selected_party_index,
            selected_tab: CharacterTab::Abilities,
        }
    }
}
