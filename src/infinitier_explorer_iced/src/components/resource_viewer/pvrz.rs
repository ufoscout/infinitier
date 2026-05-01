use iced::Element;

use crate::state::Message;

pub struct PvrzViewer;

impl PvrzViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Pvrz Viewer").into()
    }
}
