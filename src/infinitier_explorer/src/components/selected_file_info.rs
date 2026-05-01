use eframe::egui;
use log::error;

use crate::state::AppState;

pub struct SelectedFileInfo;

impl SelectedFileInfo {
    pub fn show(ui: &mut egui::Ui, state: &AppState) {
        match &state.selected_resource {
            Some(resource) => {
                if let Some(resource) = state.game_data.get_by_id(*resource) {
                    ui.label(format!("Resource: {} - Source: {:?}", resource.name, resource.data_origin));
                } else {
                    error!("Resource not found: {resource:?}");
                }
            }
            None => {
                ui.label("No file selected");
            }
        }
    }
}
