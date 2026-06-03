//! Resistances tab — read-only view of a creature's combat
//! resistances, saving throws and per-damage-type armor-class
//! modifiers.
//!
//! [`data`] pulls the values out of the CRE header; [`view`] paints
//! EEKeeper's three group boxes (Resistances · Saving Throws · Armor
//! Class Modifiers).

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct ResistancesTab;

impl ResistancesTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, _gam: &ImportedGam, _game: Game) {
        let data = data::resist_data(cre);
        view::render(ui, &data);
    }
}
