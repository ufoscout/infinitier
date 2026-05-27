//! Left rail — resource tree grouped by file extension. Title row
//! (theme-toggle button + "Resources" label) on top of a virtualized
//! tree body; scrolling is handled by `uniform_list` inside the tree
//! view itself, so this module just gives it a flex slot.

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{ActiveTheme, Sizable, Theme, ThemeMode, button::Button, h_flex, v_flex};

use crate::app::ExplorerApp;
use crate::components::key_file_tree_view;

pub fn render(this: &mut ExplorerApp, cx: &mut Context<ExplorerApp>) -> impl IntoElement {
    // Copy out the few colors we need before the mutable `cx` borrow
    // that `theme_toggle_button` takes. `Hsla` is `Copy`.
    let sidebar = cx.theme().sidebar;
    let sidebar_border = cx.theme().sidebar_border;

    v_flex()
        .w(px(260.))
        .h_full()
        .min_h_0()
        .bg(sidebar)
        .border_r_1()
        .border_color(sidebar_border)
        .child(
            h_flex()
                .w_full()
                .px_3()
                .py_2()
                .gap_2()
                .items_center()
                .child(theme_toggle_button(cx))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .child("Resources"),
                ),
        )
        .child(div().h_px().bg(sidebar_border))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .px_2()
                .py_2()
                .child(key_file_tree_view::render(this, cx)),
        )
}

/// Top-left button that cycles `ThemeMode` between `Light` and `Dark`.
/// Label reflects the *current* mode so the button reads as a switch
/// rather than a destination — same shape `keeper_gpui` uses.
fn theme_toggle_button(cx: &mut Context<ExplorerApp>) -> Button {
    let label = match cx.theme().mode {
        ThemeMode::Dark => "☀ Light",
        ThemeMode::Light => "🌙 Dark",
    };
    Button::new("theme-toggle")
        .label(label)
        .small()
        .on_click(cx.listener(|_, _, window, cx| {
            let next = match cx.theme().mode {
                ThemeMode::Dark => ThemeMode::Light,
                ThemeMode::Light => ThemeMode::Dark,
            };
            log::info!("Switching theme to {next:?}");
            // `Theme::change` swaps the global Theme + calls
            // `window.refresh()`, which re-runs every `Render` impl
            // in the window — no separate `cx.notify()` needed.
            Theme::change(next, Some(window), cx);
        }))
}
