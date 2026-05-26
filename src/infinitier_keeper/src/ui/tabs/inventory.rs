use eframe::egui;
use infinitier_core::resource::{Game, cre::Cre, gam::Gam};

pub struct InventoryTab;

impl InventoryTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &Gam, _game: Game) {
        ui.label("Inventory — not implemented yet.");
    }
}
