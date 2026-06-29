//! Cleric tab — read-only list of a creature's divine spellbook.
//!
//! [`data`] extracts the distinct known priest spells and their
//! memorised-copy counts from the CRE; [`view`] resolves each spell's
//! display name (SPL generic-name strref → `dialog.tlk`) and paints
//! EEKeeper's four-column table (Level · xMem · Spell · Resource).

mod data;
mod view;

use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

pub struct ClericTab;

impl ClericTab {
    /// Number of distinct divine spells the creature knows — shown as the
    /// count on the Spells tab's "Cleric" inner tab.
    pub fn count(&self, cre: &Cre) -> usize {
        data::cleric_rows(cre).len()
    }

    /// Needs `game_data` to resolve spell resrefs to display names via
    /// their SPL files and `dialog.tlk`.
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, game_data: &GameData) {
        let rows = data::cleric_rows(cre);
        view::render(ui, &rows, game_data);
    }
}
