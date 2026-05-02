use std::sync::Arc;

use floem::views::{img, label};
use floem::{AnyView, IntoView};
use infinitier_core::game::{GameData, ResourceId};

use crate::app::BmpCache;

pub fn view(id: ResourceId, game_data: &Arc<GameData>, cache: &BmpCache) -> AnyView {
    let mut cache_ref = cache.borrow_mut();
    let result = cache_ref.entry(id).or_insert_with(|| {
        let resource = game_data.get_by_id(id);
        let ds = resource.and_then(|r| r.datasource.as_ref());
        match ds {
            None => Err("no datasource available".to_string()),
            Some(ds) => ds
                .reader()
                .map_err(|e| e.to_string())
                .and_then(|mut r| {
                    let mut bytes = Vec::new();
                    r.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                    Ok(bytes)
                }),
        }
    });

    match result {
        Err(msg) => {
            let msg = msg.clone();
            label(move || format!("Error loading BMP: {msg}")).into_any()
        }
        Ok(bytes) => {
            let bytes = bytes.clone();
            img(move || bytes.clone()).into_any()
        }
    }
}
