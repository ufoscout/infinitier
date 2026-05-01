use iced::Element;

use crate::state::Message;

pub struct WedViewer;

impl WedViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Wed Viewer").into()
    }
}
