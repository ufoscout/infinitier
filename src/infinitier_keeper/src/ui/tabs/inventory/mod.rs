//! The "Inventory" tab: a creature's equipped and carried items,
//! read-only.
//!
//! The item slots and item list live in the CRE; each item's name and
//! inventory icon are resolved from its `.itm` file (and `dialog.tlk`),
//! so this tab takes `GameData`.

mod view;

use eframe::egui;

use infinitier_core::game::GameData;
use infinitier_core::imported_resource::cre::ImportedCre;

pub struct InventoryTab;

impl InventoryTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &ImportedCre, game_data: &GameData) {
        let rows = cre.inventory(game_data.game());
        view::render(ui, &rows, game_data);
    }
}
