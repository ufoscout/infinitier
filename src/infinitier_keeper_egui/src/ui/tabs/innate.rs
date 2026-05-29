use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct InnateTab;

impl InnateTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &ImportedGam, _game: Game) {
        ui.label("Innate — not implemented yet.");
    }
}
