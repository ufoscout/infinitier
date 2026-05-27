//! Root view — implements the `Render` trait that GPUI calls on every
//! frame. Holds the loaded game state, the cached resource viewer, and
//! the pre-built tree-view component. Mirrors the egui `ExplorerApp`
//! struct layout (state + central/left/bottom panels) one-for-one.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::components::key_file_tree_view::KeyFileTreeView;
use crate::components::resource_viewer::ResourceViewer;
use crate::state::AppState;
use crate::ui::{bottom_panel, central_panel, left_panel};

pub struct ExplorerApp {
    pub state: AppState,
    pub tree_view: KeyFileTreeView,
    pub viewer: ResourceViewer,
}

impl ExplorerApp {
    pub fn new(state: AppState, cx: &mut Context<Self>) -> Self {
        let tree_view = KeyFileTreeView::new(&state.game_data, cx);
        Self {
            state,
            tree_view,
            viewer: ResourceViewer::new(),
        }
    }
}

impl Render for ExplorerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(left_panel::render(self, cx))
                    .child(div().w_px().bg(cx.theme().border))
                    .child(central_panel::render(self, window, cx)),
            )
            .child(div().h_px().bg(cx.theme().border))
            .child(bottom_panel::render(self, cx))
    }
}
