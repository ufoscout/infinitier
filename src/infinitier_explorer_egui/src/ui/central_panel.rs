use eframe::egui;

use crate::components::resource_viewer::ResourceViewer;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, viewer: &mut ResourceViewer, state: &AppState) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        viewer.show(ui, state);
    });
}
