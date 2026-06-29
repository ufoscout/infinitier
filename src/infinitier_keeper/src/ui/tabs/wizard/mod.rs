//! Wizard tab — read-only list of a creature's arcane spellbook.
//!
//! [`data`] extracts the distinct known wizard spells and their
//! memorised-copy counts from the CRE; [`view`] resolves each spell's
//! display name (SPL generic-name strref → `dialog.tlk`) and paints
//! EEKeeper's four-column table (Level · xMem · Spell · Resource).

mod data;
mod view;

use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

pub struct WizardTab;

impl WizardTab {
    /// Number of distinct arcane spells the creature knows — shown as the
    /// count on the Spells tab's "Wizard" inner tab.
    pub fn count(&self, cre: &Cre) -> usize {
        data::wizard_rows(cre).len()
    }

    /// Needs `game_data` to resolve spell resrefs to display names via
    /// their SPL files and `dialog.tlk`. Returns the resref of a spell whose
    /// "Delete" action was chosen this frame.
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, game_data: &GameData) -> Option<String> {
        let rows = data::wizard_rows(cre);
        view::render(ui, &rows, game_data)
    }
}
