//! Top button bar.
//!
//! Left-aligned action cluster (Load, Save, …) and a right-aligned
//! theme-toggle pill. The save-game metadata that used to live here
//! now lives in the window title (see `KeeperState::window_title`),
//! so the bar is purely actions.

use gpui::{Context, IntoElement, ParentElement, Styled};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_component::{ActiveTheme, Sizable, Theme, ThemeMode, h_flex};

use crate::app::KeeperApp;
use crate::ui::save_action;

pub fn render(_this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    let secondary = cx.theme().secondary;
    let border = cx.theme().border;

    h_flex()
        .w_full()
        .px_4()
        .py_3()
        .gap_2()
        .items_center()
        .bg(secondary)
        .border_b_1()
        .border_color(border)
        .child(render_load_button(cx))
        .child(save_action::render_save_button(cx))
        // `flex_1 + justify_end` consumes the slack between the left
        // action cluster and the right edge so the theme toggle sits
        // pinned to the right regardless of window width.
        .child(
            h_flex()
                .flex_1()
                .justify_end()
                .child(theme_toggle_button(cx)),
        )
}

/// Placeholder for the in-app save picker. The action isn't wired
/// yet — the user explicitly flagged more buttons coming, so the
/// button stays here so the bar's structure is ready for it.
fn render_load_button(cx: &mut Context<KeeperApp>) -> Button {
    Button::new("keeper-load")
        .label("Load")
        .with_variant(ButtonVariant::Primary)
        .small()
        .on_click(cx.listener(|_, _, _, _| {
            log::info!("[load] Load button clicked — action not yet implemented");
        }))
}

/// Top-right pill that cycles `ThemeMode` between `Light` and `Dark`.
/// Label reflects the *current* mode so the button reads as a switch
/// rather than a destination.
fn theme_toggle_button(cx: &mut Context<KeeperApp>) -> Button {
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
            // `Theme::change` swaps the global `Theme` resource +
            // calls `window.refresh()` — that re-runs every `Render`
            // impl in the window, so we don't need an extra
            // `cx.notify()` here.
            Theme::change(next, Some(window), cx);
        }))
}
