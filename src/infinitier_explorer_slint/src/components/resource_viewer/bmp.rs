use infinitier_core::game::ResourceId;

use crate::state::AppState;

use super::ViewerData;

pub struct BmpViewer;

impl BmpViewer {
    pub fn data(state: &AppState, resource_id: ResourceId) -> ViewerData {
        match &state.bmp_cache {
            Some((cached_id, result)) if *cached_id == resource_id => match result {
                Err(msg) => ViewerData::Text(format!("Error loading BMP: {msg}").into()),
                Ok(image) => ViewerData::Image(image.clone()),
            },
            _ => ViewerData::Text("Loading...".into()),
        }
    }
}
