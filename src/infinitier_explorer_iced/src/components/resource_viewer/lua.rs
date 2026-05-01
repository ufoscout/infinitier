use iced::Element;

use crate::state::Message;

pub struct LuaViewer;

impl LuaViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Lua Viewer").into()
    }
}
