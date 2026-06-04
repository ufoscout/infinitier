//! Miscellaneous tab — read-only view of a creature's scripts /
//! tracking / identifier plus the save's world / game / joined-party
//! clocks.
//!
//! [`data`] pulls the values from the CRE header, the GAM `game_time`
//! and the party slot's join time; [`view`] paints EEKeeper's "Other"
//! card alongside the three day/hour/minute time cards.

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::{ImportedGam, ImportedGamNpc};
use infinitier_core::resource::cre::Cre;

pub struct MiscellaneousTab;

impl MiscellaneousTab {
    /// Needs `gam` for the save clock and `npc` for the party slot's
    /// join time.
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, gam: &ImportedGam, npc: &ImportedGamNpc) {
        let data = data::misc_data(cre, gam, npc);
        view::render(ui, &data);
    }
}
