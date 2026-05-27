//! "Resource: NAME — Source: ORIGIN" line painted in the bottom bar.
//! Mirrors the egui `SelectedFileInfo`. Pure render function — no
//! interactivity, so the signature only needs `&ExplorerApp`.

use log::error;

use crate::app::ExplorerApp;

pub fn render(this: &ExplorerApp) -> String {
    match &this.state.selected_resource {
        Some(resource_id) => match this.state.game_data.get_by_id(*resource_id) {
            Some(resource) => format!(
                "Resource: {} — Source: {:?}",
                resource.name, resource.data_origin
            ),
            None => {
                error!("Resource not found: {resource_id:?}");
                "Resource not found".into()
            }
        },
        None => "No file selected".into(),
    }
}
