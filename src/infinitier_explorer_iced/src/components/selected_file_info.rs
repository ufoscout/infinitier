use iced::Element;
use log::error;

use infinitier_core::game::DataOrigin;

use crate::state::{AppState, Message};

pub struct SelectedFileInfo;

impl SelectedFileInfo {
    pub fn view<'a>(state: &'a AppState) -> Element<'a, Message> {
        match state.selected {
            Some(resource_id) => {
                if let Some(resource) = state.game_data.get_by_id(resource_id) {
                    let origin = match &resource.data_origin {
                        DataOrigin::Bif { name } => format!("BIF: {name}"),
                        DataOrigin::Override { path } => {
                            format!("Override: {}", path.display())
                        }
                        DataOrigin::Missing => "Missing".to_string(),
                    };
                    iced::widget::text(format!("Resource: {} — Source: {}", resource.name, origin))
                        .into()
                } else {
                    error!("Resource not found: {resource_id:?}");
                    iced::widget::text("Resource not found").into()
                }
            }
            None => iced::widget::text("No file selected").into(),
        }
    }
}
