//! App-wide mutable state, separated from the egui scaffolding so
//! the UI panels can borrow it without coupling to the eframe shell.
//!
//! Shape:
//! - [`AppState`] owns the *game-wide* context (the resolved
//!   [`GameData`] index and the engine-derived caps + 2DA bonus
//!   tables) plus a `Vec<SaveTab>` of opened save games and the
//!   index of the active tab.
//! - [`SaveTab`] holds the *per-save* state — the on-disk folder
//!   path, the loaded GAM (with edits-in-flight), the currently
//!   selected party slot, and which character sub-tab is showing.
//!
//! All UI panels still address a single save at a time; instead of
//! reading `state.save` they now read `state.active().save`. The
//! borrow rules behave the same — the split exists so future code
//! can open additional saves into new tabs without restarting the
//! keeper.

use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::save_games::SaveGame;

use crate::ui::CharacterTab;

/// Top-level keeper state.
pub struct AppState {
    /// Pre-indexed game data — the FS, `chitin.key` index, and every
    /// resource the engine reaches. Shared across every open save in
    /// this keeper instance because they all came from the same
    /// game install.
    pub game_data: GameData,
    /// Cap ranges + bonus tables for the active engine. Built once
    /// at startup via [`EngineCaps::new`], which reads `STRMOD.2DA`
    /// / `STRMODEX.2DA` / `DEXMOD.2DA` / `HPCONBON.2DA` so modded
    /// installs get their actual numbers instead of the vanilla
    /// distribution.
    pub engine_caps: EngineCaps,
    /// One entry per opened save. Currently a single tab is opened
    /// at startup (from the CLI args); the Load button will append
    /// more once it's wired.
    pub tabs: Vec<SaveTab>,
    /// Index into [`Self::tabs`] of the tab whose content the rest
    /// of the UI is showing. Always in range — the constructors and
    /// any tab-mutation path keep this invariant.
    pub active_tab: usize,
}

/// Per-save state for one open save game.
pub struct SaveTab {
    /// Reference to the save game on disk.
    pub save_game: SaveGame,
    /// Loaded save state, with every embedded CRE parsed and every
    /// NPC name resolved through `dialog.tlk` when one was
    /// reachable.
    pub save: Box<ImportedGam>,
    /// Selected party-member row, or `None` until the user clicks
    /// one.
    pub selected_party_index: Option<usize>,
    /// Currently-active sub-tab in the per-character editor on the
    /// right.
    pub selected_tab: CharacterTab,
}

impl AppState {
    /// Window title for the keeper. Carries the game type only —
    /// per-save data (save name) lives in the tab strip below the
    /// header.
    pub fn window_title(&self) -> String {
        format!("Infinitier Keeper - {:?}", self.game_data.game())
    }

    pub fn new(game_data: GameData, engine_caps: EngineCaps) -> Self {
        Self {
            game_data,
            engine_caps,
            tabs: vec![],
            active_tab: 0,
        }
    }

    /// Shared borrow of the active save's per-tab state.
    pub fn active(&self) -> &SaveTab {
        &self.tabs[self.active_tab]
    }

    /// Mutable borrow of the active save's per-tab state. Holding
    /// this mutably locks the entire [`AppState`], so call sites
    /// that also need `&self.engine_caps` should split-borrow over
    /// `AppState`'s fields (`let AppState { engine_caps, tabs,
    /// active_tab, .. } = &mut *state;`) instead.
    pub fn active_mut(&mut self) -> &mut SaveTab {
        &mut self.tabs[self.active_tab]
    }
}

impl SaveTab {
    pub fn new(save_game: SaveGame, save: Box<ImportedGam>) -> Self {
        let selected_party_index = if save.party_npcs.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            save_game,
            save,
            selected_party_index,
            selected_tab: CharacterTab::Abilities,
        }
    }
}
