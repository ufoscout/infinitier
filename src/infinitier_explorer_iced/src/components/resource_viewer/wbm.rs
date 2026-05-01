use iced::Element;

use crate::state::Message;

pub struct WbmViewer;

impl WbmViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Wbm Viewer").into()
    }
}
