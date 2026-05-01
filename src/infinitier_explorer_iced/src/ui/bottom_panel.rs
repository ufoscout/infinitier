use iced::widget::container;
use iced::{Element, Length};

use crate::components::selected_file_info::SelectedFileInfo;
use crate::state::{AppState, Message};

pub fn view<'a>(state: &'a AppState) -> Element<'a, Message> {
    container(SelectedFileInfo::view(state))
        .padding(4)
        .width(Length::Fill)
        .into()
}
