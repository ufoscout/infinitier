//! Root view — implements the `Render` trait that GPUI calls on every
//! frame for the top-of-window entity. Holds the loaded game state +
//! the two pieces of mutable UI state (selected party slot, selected
//! tab) that the panel modules read and the listener closures write.

use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Window, div,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::state::KeeperState;
use crate::ui::tabs::CharacterTab;
use crate::ui::{character, header, party};

pub struct KeeperApp {
    pub state: KeeperState,
    pub selected_party: Option<usize>,
    pub selected_tab: CharacterTab,
}

impl KeeperApp {
    pub fn new(state: KeeperState) -> Self {
        let selected_party = if state.imported_gam.party_npcs.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            state,
            selected_party,
            selected_tab: CharacterTab::Abilities,
        }
    }
}

impl Render for KeeperApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(header::render(self, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(party::render(self, cx))
                    .child(div().w_px().bg(cx.theme().border))
                    .child(character::render(self, cx)),
            )
    }
}
