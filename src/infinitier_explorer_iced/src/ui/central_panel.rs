use iced::widget::container;
use iced::{Element, Length};

use crate::components::resource_viewer::ResourceViewer;
use crate::state::{AppState, Message};

pub fn view<'a>(state: &'a AppState, viewer: &'a ResourceViewer) -> Element<'a, Message> {
    container(viewer.view(state))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
