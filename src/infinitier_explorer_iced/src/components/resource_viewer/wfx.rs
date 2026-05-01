use iced::Element;

use crate::state::Message;

pub struct WfxViewer;

impl WfxViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Wfx Viewer").into()
    }
}
