use eframe::egui;

use crate::components::resource_viewer::ResourceViewer;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &AppState) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ResourceViewer::show(ui, state);
    });
}
