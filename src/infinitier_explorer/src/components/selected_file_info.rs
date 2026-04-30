use eframe::egui;

use crate::state::AppState;

pub struct SelectedFileInfo;

impl SelectedFileInfo {
    pub fn show(ui: &mut egui::Ui, state: &AppState) {
        match &state.selected_resource {
            Some(resource) => {
                let filename = state
                    .key_file
                    .bif_entries
                    .get(resource.bif_entries_index as usize)
                    .map(|entry| entry.file_name.as_str())
                    .unwrap_or_default();
                ui.label(format!("Path: {filename} - Selected: {resource:?}"));
            }
            None => {
                ui.label("No file selected");
            }
        }
    }
}
