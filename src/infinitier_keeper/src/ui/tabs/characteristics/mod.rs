//! Characteristics tab — read-only view of a creature's identity
//! (gender / race / alignment / class / kit / …), its GAM-sourced
//! kill statistics, and the creature-flags "Miscellaneous" checkboxes.
//!
//! [`data`] handles extraction + IDS/TLK name resolution; [`view`]
//! paints the EEKeeper-style two-column layout.

mod data;
mod view;

use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::cre::ImportedCre;
use infinitier_core::imported_resource::gam::ImportedGamNpc;

use data::CharData;

pub struct CharacteristicsTab;

impl CharacteristicsTab {
    /// `npc` is the GAM party slot — its typed `char_stats` block
    /// carries the kill statistics, which don't live in the CRE.
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        cre: &ImportedCre,
        npc: &ImportedGamNpc,
        game_data: &GameData,
    ) {
        let data = CharData::resolve(cre, &npc.char_stats, game_data);
        view::render(ui, &data, game_data);
    }
}
