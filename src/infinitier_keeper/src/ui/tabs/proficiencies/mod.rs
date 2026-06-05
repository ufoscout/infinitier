//! Proficiencies tab — read-only weapon-proficiency table.
//!
//! [`data`] extracts the first/second-class points (CRE header block
//! plus `op233` proficiency effects); [`view`] paints EEKeeper's
//! three-column table.

mod data;
mod view;

use eframe::egui;
use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::resource::cre::Cre;

pub struct ProficienciesTab;

impl ProficienciesTab {
    /// The proficiency list (stats + display names) is resolved once at
    /// startup into `engine_caps` from the game's `WEAPPROF.2DA`; the tab
    /// just pairs it with the creature's points.
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, engine_caps: &EngineCaps) {
        let rows = data::proficiency_rows(cre, engine_caps.proficiencies());
        view::render(ui, &rows);
    }
}
