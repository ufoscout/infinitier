//! Innate tab — read-only list of a creature's innate abilities.
//!
//! [`data`] extracts the distinct known innate spells and their
//! memorised-copy counts from the CRE; [`view`] resolves each spell's
//! display name (SPL generic-name strref → `dialog.tlk`) and paints
//! EEKeeper's four-column table (Level · xMem · Spell · Resource).

mod data;
mod view;

use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

pub struct InnateTab;

impl InnateTab {
    /// Needs `game_data` to resolve spell resrefs to display names via
    /// their SPL files and `dialog.tlk`.
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, game_data: &GameData) {
        let rows = data::innate_rows(cre);
        view::render(ui, &rows, game_data);
    }
}
