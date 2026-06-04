//! Effects tab — read-only list of a creature's EE effect records.
//!
//! [`data`] extracts the V2 effect rows (excluding the proficiency /
//! local-variable opcodes that have their own tabs); [`opcode_names`]
//! is the generated opcode → display-name table; [`view`] resolves
//! resources / timing / target and paints EEKeeper's wide table.

mod data;
mod opcode_names;
mod view;

use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

pub struct EffectsTab;

impl EffectsTab {
    /// Needs `game_data` to resolve resource resrefs to spell names via
    /// their SPL files and `dialog.tlk`.
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, game_data: &GameData) {
        let rows = data::effect_rows(cre);
        view::render(ui, &rows, game_data);
    }
}
