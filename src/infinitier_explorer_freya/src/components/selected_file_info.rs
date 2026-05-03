use freya::prelude::*;
use infinitier_core::game::DataOrigin;
use log::error;

use crate::state::AppState;

#[derive(PartialEq)]
pub struct SelectedFileInfo;

impl Component for SelectedFileInfo {
    fn render(&self) -> impl IntoElement {
        let state = use_consume::<State<AppState>>();
        let selected = state.read().selected;
        let text = match selected {
            None => "No file selected".to_string(),
            Some(resource_id) => {
                if let Some(resource) = state.read().game_data.get_by_id(resource_id) {
                    let origin = match &resource.data_origin {
                        DataOrigin::Bif { name } => format!("BIF: {name}"),
                        DataOrigin::Override { path } => format!("Override: {}", path.display()),
                        DataOrigin::Missing => "Missing".to_string(),
                    };
                    format!("Resource: {} — Source: {}", resource.name, origin)
                } else {
                    error!("Resource not found: {resource_id:?}");
                    "Resource not found".to_string()
                }
            }
        };
        label().text(text).font_size(13.)
    }
}
