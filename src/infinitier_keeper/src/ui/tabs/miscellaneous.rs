use eframe::egui;
use infinitier_common::Game;
use infinitier_cre_resource::Cre;
use infinitier_gam_resource::Gam;

pub struct MiscellaneousTab;

impl MiscellaneousTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &Gam, _game: Game) {
        ui.label("Miscellaneous — not implemented yet.");
    }
}
