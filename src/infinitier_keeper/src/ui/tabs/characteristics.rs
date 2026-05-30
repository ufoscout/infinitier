use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct CharacteristicsTab;

impl CharacteristicsTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &ImportedGam, _game: Game) {
        ui.label("Characteristics — not implemented yet.");
    }
}
