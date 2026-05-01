use iced::Element;

use crate::state::Message;

pub struct MosViewer;

impl MosViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Mos Viewer").into()
    }
}
