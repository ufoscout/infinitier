use iced::Element;

use crate::state::Message;

pub struct WmpViewer;

impl WmpViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Wmp Viewer").into()
    }
}
