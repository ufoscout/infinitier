use iced::Element;

use crate::state::Message;

pub struct VefViewer;

impl VefViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Vef Viewer").into()
    }
}
