use xilem::{AnyWidgetView, WidgetView};
use xilem::view::{image, label};
use infinitier_core::game::ResourceId;

use crate::state::AppState;

pub fn view(state: &AppState, resource_id: ResourceId) -> Box<AnyWidgetView<AppState>> {
    match &state.bmp_cache {
        Some((cached_id, result)) if *cached_id == resource_id => match result {
            Err(msg) => label(format!("Error loading BMP: {msg}")).boxed(),
            Ok(img) => image(img.clone()).boxed(),
        },
        _ => label("Loading...").boxed(),
    }
}
