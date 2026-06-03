//! Proficiencies tab — read-only weapon-proficiency table.
//!
//! [`data`] extracts the first/second-class points (CRE header block
//! + `op233` proficiency effects); [`view`] paints EEKeeper's
//! three-column table.

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct ProficienciesTab;

impl ProficienciesTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, _gam: &ImportedGam, _game: Game) {
        let rows = data::proficiency_rows(cre);
        view::render(ui, &rows);
    }
}
