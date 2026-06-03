//! Memorization tab — read-only spell-slot table.
//!
//! [`data`] extracts one row per spell-memorisation slot from the CRE
//! (`spell_memorization_info`); [`view`] paints EEKeeper's
//! three-column table (Type · Level · Max Can Memorise).

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct MemorizationTab;

impl MemorizationTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, _gam: &ImportedGam, _game: Game) {
        let rows = data::memorization_rows(cre);
        view::render(ui, &rows);
    }
}
