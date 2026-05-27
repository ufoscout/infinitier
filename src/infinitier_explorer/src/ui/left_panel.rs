use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_egui_common::theme;

use crate::components::key_file_tree_view::KeyFileTreeView;
use crate::state::AppState;

pub struct LeftPanel {
    tree_view: KeyFileTreeView,
}

impl LeftPanel {
    pub fn new(game_data: &GameData) -> Self {
        Self {
            tree_view: KeyFileTreeView::new(game_data),
        }
    }

    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        egui::Panel::left("resource_panel")
            .resizable(true)
            .default_size(260.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    theme_toggle_button(ui);
                    ui.heading("Resources");
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.tree_view.show(ui, state);
                });
            });
    }
}

/// Small button at the top-left that cycles between the two palettes
/// in `infinitier_egui_common::theme`. Label reflects the mode the
/// click would switch *to*, matching the keeper_gpui pattern.
fn theme_toggle_button(ui: &mut egui::Ui) {
    let palette = theme::active();
    let label = if palette.dark_mode { "Light" } else { "Dark" };
    if ui.small_button(label).clicked() {
        let next = if palette.dark_mode {
            &theme::LIGHT
        } else {
            &theme::DARK
        };
        theme::apply(ui.ctx(), next);
    }
}
