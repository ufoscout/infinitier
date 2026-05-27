//! Top-of-window metadata strip — matches the Slint `HeaderPanel`.
//! Four caption / value columns on a single `secondary` band, plus a
//! "Toggle theme" button anchored at the top-left that flips between
//! the registered Light and Dark themes via `Theme::change`.

use gpui::{Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_component::{ActiveTheme, Sizable, Theme, ThemeMode, button::Button, h_flex, v_flex};

use crate::app::KeeperApp;

pub fn render(this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    let roots = this
        .state
        .game_data
        .fs()
        .get_roots()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let secondary = cx.theme().secondary;
    let border = cx.theme().border;

    h_flex()
        .w_full()
        .px_4()
        .py_3()
        .gap_6()
        .items_center()
        .bg(secondary)
        .border_b_1()
        .border_color(border)
        .child(theme_toggle_button(cx))
        .child(field(cx, "Game", format!("{:?}", this.state.game_data.game())))
        .child(field(cx, "Folder", roots))
        .child(field(cx, "Save", this.state.save_name.clone()))
        .child(field(
            cx,
            "GAM",
            format!("{:?}", this.state.imported_gam.version),
        ))
}

/// Top-left button that cycles `ThemeMode` between `Light` and `Dark`.
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

/// One caption + value column. Matches the Slint `HeaderField`
/// component: muted 11-px caption above a bold 14-px value.
fn field(
    cx: &Context<KeeperApp>,
    caption: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> impl IntoElement {
    v_flex()
        .gap_0p5()
        .child(
            div()
                .text_size(px(11.))
                .text_color(cx.theme().muted_foreground)
                .child(caption.into()),
        )
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .child(value.into()),
        )
}
