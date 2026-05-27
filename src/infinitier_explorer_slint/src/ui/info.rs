//! Bottom info-bar population. Mirrors the egui `SelectedFileInfo`
//! exactly: shows the resource name and data origin, or a "No file
//! selected" placeholder.

use std::rc::Rc;

use crate::MainWindow;
use crate::state::AppState;

pub fn clear(window: &MainWindow) {
    window.set_info_message("No file selected".into());
}

pub fn show(window: &MainWindow, state: &Rc<AppState>, resource_idx: usize) {
    let msg = match state.game_data.get_by_id(resource_idx) {
        Some(r) => format!("Resource: {} - Source: {:?}", r.name, r.data_origin),
        None => format!("Resource not found: id={resource_idx}"),
    };
    window.set_info_message(msg.into());
}
