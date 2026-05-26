use eframe::egui;
use infinitier_core::resource::{Game, cre::Cre, gam::Gam};

pub struct ProficienciesTab;

impl ProficienciesTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &Gam, _game: Game) {
        ui.label("Proficiencies — not implemented yet.");
    }
}
