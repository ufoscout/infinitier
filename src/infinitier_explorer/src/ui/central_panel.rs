//! Central area — hosts the per-resource viewer. Modelled as a flex
//! column so children with `flex_1 + min_h_0` (the image viewer's
//! picture area) get a definite vertical slot to fill, the way
//! `infinitier_keeper_gpui::ui::character` does it.

use gpui::{Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{ActiveTheme, v_flex};

use crate::app::ExplorerApp;
use crate::components::resource_viewer;

pub fn render(
    this: &mut ExplorerApp,
    window: &mut Window,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
        .p_3()
        .bg(cx.theme().background)
        .child(resource_viewer::render(this, window, cx))
}
