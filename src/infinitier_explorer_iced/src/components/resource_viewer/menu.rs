use iced::Element;

use crate::state::Message;

pub struct MenuViewer;

impl MenuViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Menu Viewer").into()
    }
}
