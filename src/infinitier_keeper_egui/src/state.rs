//! App-wide mutable state, separated from the egui scaffolding so
//! the UI panels can borrow it without coupling to the eframe shell.

use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

use crate::ui::CharacterTab;

/// Top-level keeper state. Loaded once at startup; refreshed on
/// reload (future work — for now the loader runs in `main`).
pub struct AppState {
    /// Pre-indexed game data — the FS, `chitin.key` index, and every
    /// resource the engine reaches. Used by tabs that need to resolve
    /// 2DAs, item names, spell descriptions, etc.
    pub game_data: GameData,
    /// On-disk name of the open save folder (the
    /// [`infinitier_core::save_games::SaveGame::name`] the user
    /// picked at startup). Kept on the side because [`ImportedGam`]
    /// has no notion of "where on disk did this come from" — the
    /// name lives on the enclosing save folder, not the GAM file.
    pub save_name: String,
    /// Loaded save state, with every embedded CRE parsed and every
    /// NPC name resolved through `dialog.tlk` when one was
    /// reachable.
    pub save: Box<ImportedGam>,
    /// Selected party-member row, or `None` until the user clicks
    /// one.
    pub selected_party_index: Option<usize>,
    /// Currently-active tab in the per-character editor on the right.
    pub selected_tab: CharacterTab,
}

impl AppState {
    pub fn new(game_data: GameData, save_name: String, save: Box<ImportedGam>) -> Self {
        let selected_party_index = if save.party_npcs.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            game_data,
            save_name,
            save,
            selected_party_index,
            selected_tab: CharacterTab::Abilities,
        }
    }
}
