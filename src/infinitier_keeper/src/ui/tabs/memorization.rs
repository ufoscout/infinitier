use eframe::egui;
use infinitier_core::resource::{Game, cre::Cre, gam::Gam};

pub struct MemorizationTab;

impl MemorizationTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &Gam, _game: Game) {
        ui.label("Memorization — not implemented yet.");
    }
}
