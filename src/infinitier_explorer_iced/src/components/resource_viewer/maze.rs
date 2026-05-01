use iced::Element;

use crate::state::Message;

pub struct MazeViewer;

impl MazeViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Maze Viewer").into()
    }
}
