use eframe::egui;

use crate::components::selected_file_info::SelectedFileInfo;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &AppState) {
    egui::Panel::bottom("info_panel")
        .resizable(false)
        .show_inside(ui, |ui| {
            SelectedFileInfo::show(ui, state);
        });
}
