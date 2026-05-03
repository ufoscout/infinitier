use freya::prelude::*;
use infinitier_core::game::ResourceId;

use crate::state::AppState;

pub fn bmp_viewer(state: State<AppState>, resource_id: ResourceId) -> Element {
    let cache = state.read().bmp_cache.as_ref().and_then(|(cid, result)| {
        if *cid == resource_id {
            Some(result.clone())
        } else {
            None
        }
    });

    match cache {
        None => label().text("Loading...").into(),
        Some(Err(msg)) => label().text(format!("Error loading BMP: {msg}")).into(),
        Some(Ok(bytes)) => ImageViewer::new((resource_id, bytes))
            .width(Size::fill())
            .height(Size::fill())
            .into(),
    }
}
