use iced::Element;

use crate::state::Message;

pub struct GuiViewer;

impl GuiViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Gui Viewer").into()
    }
}
