//! Local Variables tab — read-only list of a creature's per-creature
//! script variables (`LOCALS` scope).
//!
//! The variables are parsed by the CRE importer (stored as `op187`
//! effects, surfaced as [`infinitier_core::resource::cre::LocalVariable`]);
//! this tab just collects them in file order and [`view`] paints
//! EEKeeper's two-column table (Name · Value).

mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct LocalVariablesTab;

impl LocalVariablesTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, _gam: &ImportedGam, _game: Game) {
        let vars: Vec<_> = cre.local_variables().collect();
        view::render(ui, &vars);
    }
}
