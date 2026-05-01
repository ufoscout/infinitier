use iced::Element;

use crate::state::Message;

pub struct ItmViewer;

impl ItmViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Itm Viewer").into()
    }
}
