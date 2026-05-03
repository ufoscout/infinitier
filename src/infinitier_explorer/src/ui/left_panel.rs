use eframe::egui;
use infinitier_core::game::GameData;

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
                ui.heading("Resources");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.tree_view.show(ui, state);
                });
            });
    }
}
