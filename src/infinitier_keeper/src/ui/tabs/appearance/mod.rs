//! The "Appearance" tab: a creature's animation and the seven colour
//! gradients (hair, skin, clothing, armour, leather, metal), read-only.
//!
//! The animation id is resolved through ANIMATE.IDS and the colour
//! swatches through the palette BMP, so this tab takes `GameData`.

mod data;
mod palette;
mod view;

use eframe::egui;

use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

use egui_components::Label;

pub struct AppearanceTab;

impl AppearanceTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, game_data: &GameData) {
        match data::appearance_data(cre) {
            Some(appearance) => view::render(ui, &appearance, game_data),
            None => {
                ui.add(Label::new(
                    "Appearance is unavailable for this creature's format.",
                ));
            }
        }
    }
}
