//! Global Variables tab — read-only list of the save's GLOBAL script
//! variables.
//!
//! Globals live in the GAM (once per save, shared by the whole party),
//! not in any single creature — so this tab reads from `gam` and shows
//! the same list regardless of which party member is selected.
//!
//! [`data`] pulls + sorts the variables; [`view`] paints EEKeeper's
//! two-column table (Name · Value).

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

pub struct GlobalVariablesTab;

impl GlobalVariablesTab {
    pub fn show(&self, ui: &mut egui::Ui, _cre: &Cre, gam: &ImportedGam, _game: Game) {
        let rows = data::global_variable_rows(gam);
        view::render(ui, &rows);
    }
}
