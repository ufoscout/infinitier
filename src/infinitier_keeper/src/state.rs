//! Loaded keeper state. Owned by the root `KeeperApp` view; mutated
//! through `cx.listener` closures when the user clicks a party row, a
//! tab chip, or a save-tab in the strip.
//!
//! Shape:
//! - [`KeeperState`] owns the *game-wide* context (the resolved
//!   [`GameData`] index and the engine-derived caps + 2DA bonus
//!   tables) plus a `Vec<SaveTab>` of opened save games and the
//!   index of the active tab.
//! - [`SaveTab`] holds the *per-save* state — the on-disk folder
//!   path, the loaded GAM (with edits-in-flight), the currently
//!   selected party slot, and which character sub-tab is showing.
//!
//! All UI panels still address a single save at a time; instead of
//! reading `state.imported_gam` they now read
//! `state.active().imported_gam`. The borrow rules behave the same —
//! the split exists so future code can open additional saves into
//! new tabs without restarting the keeper.

use std::path::PathBuf;

use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

use crate::ui::tabs::CharacterTab;

pub struct KeeperState {
    /// Pre-indexed game data — shared across every open save in this
    /// keeper instance.
    pub game_data: GameData,
    /// Cap ranges + 2DA-driven bonus tables for the active engine.
    /// Built once at startup via [`EngineCaps::new`] and consulted
    /// by the abilities tab for clamping and live bonus display.
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

pub struct SaveTab {
    pub save_name: String,
    /// Absolute on-disk path of the save folder the keeper opened.
    /// Used by the Save action to compute the destination folder
    /// (sibling of this one) and to enumerate the files to copy.
    pub save_folder_path: PathBuf,
    pub imported_gam: ImportedGam,
    /// Selected party-member slot, or `None` until the user clicks
    /// one (or the save is empty).
    pub selected_party: Option<usize>,
    /// Currently-active per-character sub-tab.
    pub selected_tab: CharacterTab,
}

impl KeeperState {
    /// Window title for the keeper. Carries the game type only —
    /// per-save data (save name) lives in the tab strip below the
    /// header.
    pub fn window_title(&self) -> String {
        format!("Infinitier Keeper - {:?}", self.game_data.game())
    }

    /// Shared borrow of the active save's per-tab state.
    pub fn active(&self) -> &SaveTab {
        &self.tabs[self.active_tab]
    }

    /// Mutable borrow of the active save's per-tab state. Locks
    /// the whole [`KeeperState`] mutably, so paths that also need
    /// `&self.engine_caps` should split-borrow over the struct's
    /// fields (`let KeeperState { engine_caps, tabs, active_tab,
    /// .. } = &mut state;`) instead.
    pub fn active_mut(&mut self) -> &mut SaveTab {
        &mut self.tabs[self.active_tab]
    }
}

impl SaveTab {
    pub fn new(save_name: String, save_folder_path: PathBuf, imported_gam: ImportedGam) -> Self {
        let selected_party = if imported_gam.party_npcs.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            save_name,
            save_folder_path,
            imported_gam,
            selected_party,
            selected_tab: CharacterTab::Abilities,
        }
    }
}
