use iced::Element;

use crate::state::Message;

pub struct GlslViewer;

impl GlslViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Glsl Viewer").into()
    }
}
