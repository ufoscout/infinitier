use iced::widget::Image;
use iced::{ContentFit, Element};

use infinitier_core::game::ResourceId;

use crate::state::{AppState, Message};

pub struct BmpViewer;

impl BmpViewer {
    pub fn view<'a>(state: &'a AppState, resource_id: ResourceId) -> Element<'a, Message> {
        if let Some((cached_id, result)) = &state.bmp_cache {
            if *cached_id == resource_id {
                return match result {
                    Err(msg) => iced::widget::text(format!("Error loading BMP: {msg}")).into(),
                    Ok(handle) => Image::new(handle.clone())
                        .content_fit(ContentFit::Contain)
                        .into(),
                };
            }
        }
        iced::widget::text("Loading...").into()
    }
}
