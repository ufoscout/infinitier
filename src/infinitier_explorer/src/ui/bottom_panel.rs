//! Bottom info bar — shows the currently selected resource's name and
//! data origin. Mirrors the egui `BottomPanel` / `SelectedFileInfo`
//! combo.

use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::{ActiveTheme, h_flex};

use crate::app::ExplorerApp;
use crate::components::selected_file_info;

pub fn render(this: &ExplorerApp, cx: &mut Context<ExplorerApp>) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .border_t_1()
        .border_color(theme.border)
        .child(div().child(selected_file_info::render(this)))
}
