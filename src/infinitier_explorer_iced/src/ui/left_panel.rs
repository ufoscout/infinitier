use iced::widget::{column, container, text};
use iced::{Element, Length};

use crate::components::resource_tree::ResourceTree;
use crate::state::{AppState, Message};

pub fn view<'a>(state: &'a AppState, tree: &'a ResourceTree) -> Element<'a, Message> {
    let content = column![
        text("Resources").size(16),
        iced::widget::horizontal_rule(1),
        tree.view(state),
    ]
    .spacing(4);

    container(content)
        .width(Length::Fixed(280.0))
        .height(Length::Fill)
        .padding(4)
        .into()
}
