use eframe::egui;
use infinitier_common::Game;
use infinitier_cre_resource::Cre;
use infinitier_gam_resource::Gam;

pub struct InnateTab;

impl InnateTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, _gam: &Gam, _game: Game) {
        ui.label("Innate — not implemented yet.");
    }
}
