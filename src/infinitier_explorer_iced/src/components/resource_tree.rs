use iced::widget::{button, column, scrollable, text, Column};
use iced::{Element, Length};

use infinitier_core::game::{GameData, ResourceId};

use crate::state::{AppState, Groups, Message, build_groups};

pub struct ResourceTree {
    groups: Groups,
}

impl ResourceTree {
    pub fn new(game_data: &GameData) -> Self {
        Self {
            groups: build_groups(game_data),
        }
    }

    /// Returns the resource IDs of all items currently visible in the tree (expanded groups only),
    /// in the same top-to-bottom order they appear on screen.
    pub fn visible_items(&self, state: &AppState) -> Vec<ResourceId> {
        let mut items = Vec::new();
        for (ext, entries) in &self.groups {
            if state.expanded.contains(ext) {
                for (_, resource_id) in entries {
                    items.push(*resource_id);
                }
            }
        }
        items
    }

    pub fn view<'a>(&'a self, state: &'a AppState) -> Element<'a, Message> {
        let mut col: Column<Message> = column![].spacing(0);

        for (ext, entries) in &self.groups {
            let is_open = state.expanded.contains(ext);
            let arrow = if is_open { "▼" } else { "▶" };
            let header_label = format!("{} {} ({})", arrow, ext, entries.len());

            let header_btn = button(text(header_label).size(13))
                .on_press(Message::ToggleGroup(ext.clone()))
                .style(button::text)
                .width(Length::Fill);
            col = col.push(header_btn);

            if is_open {
                for (label, resource_id) in entries {
                    let id = *resource_id;
                    let mut item_btn = button(text(format!("  {}", label)))
                        .on_press(Message::SelectResource(id))
                        .width(Length::Fill);
                    if state.selected == Some(id) {
                        item_btn = item_btn.style(button::primary);
                    } else {
                        item_btn = item_btn.style(button::text);
                    }
                    col = col.push(item_btn);
                }
            }
        }

        scrollable(col).into()
    }
}
