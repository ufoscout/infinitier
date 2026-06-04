//! The "Inventory" tab: a creature's equipped and carried items,
//! read-only.
//!
//! The item slots and item list live in the CRE; each item's name and
//! inventory icon are resolved from its `.itm` file (and `dialog.tlk`),
//! so this tab takes `GameData`.

mod data;
mod view;

use eframe::egui;

use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

pub struct InventoryTab;

impl InventoryTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, game_data: &GameData) {
        let rows = data::inventory_rows(cre);
        view::render(ui, &rows, game_data);
    }
}
