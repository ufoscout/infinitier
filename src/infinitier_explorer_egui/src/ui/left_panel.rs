use eframe::egui;

use crate::components::key_file_tree_view::KeyFileTreeView;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, tree_view: &KeyFileTreeView, state: &mut AppState) {
    egui::Panel::left("resource_panel")
        .resizable(true)
        .default_size(260.0)
        .show_inside(ui, |ui| {
            ui.heading("Resources");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                tree_view.show(ui, state);
            });
        });
}
