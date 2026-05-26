use eframe::egui;
use infinitier_core::resource::{Game, cre::Cre, gam::Gam};

pub struct InnateTab;

impl InnateTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &Gam, _game: Game) {
        ui.label("Innate — not implemented yet.");
    }
}
