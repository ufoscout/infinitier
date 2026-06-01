//! The component set. Deliberately small — only what the keeper port
//! needs — and modelled loosely on `egui_components`' surface (labels
//! with tones, a titled `card`, value rows, a tab strip built from
//! `tab_button`s). Every constructor takes a [`Theme`] by value and
//! applies its colours/metrics through Xilem's `Style` properties, so
//! the look is fully driven by the theme.

use xilem::masonry::layout::Length;
use xilem::style::{Padding, Style as _};
use xilem::view::{
    CrossAxisAlignment, FlexSpacer, MainAxisAlignment, button, divider_h, flex_col, flex_row,
    label, sized_box,
};
use xilem::{AnyWidgetView, FontWeight, WidgetView};

use crate::theme::Theme;

/// Combo box / select — re-exported here so it lives alongside the other
/// `view`-module components (`xc::select`). See [`crate::select`].
pub use crate::select::select;

fn px(v: f32) -> Length {
    Length::px(v as f64)
}

// ── Text / labels (tones mirror egui_components' LabelTone) ───────────

/// Body text in the default foreground colour.
pub fn text<State: 'static, Action: 'static>(
    theme: Theme,
    s: impl Into<String>,
) -> impl WidgetView<State, Action> {
    label(s.into())
        .text_size(theme.font_size)
        .color(theme.foreground)
}

/// Muted secondary text — used for row labels and counters.
pub fn muted<State: 'static, Action: 'static>(
    theme: Theme,
    s: impl Into<String>,
) -> impl WidgetView<State, Action> {
    label(s.into())
        .text_size(theme.font_size)
        .color(theme.muted_foreground)
}

/// Emphasised value text (bold foreground).
pub fn strong<State: 'static, Action: 'static>(
    theme: Theme,
    s: impl Into<String>,
) -> impl WidgetView<State, Action> {
    label(s.into())
        .text_size(theme.font_size)
        .weight(FontWeight::BOLD)
        .color(theme.foreground)
}

/// Card / section title.
pub fn title<State: 'static, Action: 'static>(
    theme: Theme,
    s: impl Into<String>,
) -> impl WidgetView<State, Action> {
    label(s.into())
        .text_size(theme.title_size)
        .weight(FontWeight::BOLD)
        .color(theme.foreground)
}

// ── Separators / surfaces ─────────────────────────────────────────────

/// A thin horizontal rule in the theme's border colour.
pub fn separator<State: 'static, Action: 'static>(theme: Theme) -> impl WidgetView<State, Action> {
    let _ = theme;
    divider_h().thickness(px(1.0))
}

/// A flat surface panel (used for the header / tab strips): background
/// fill + padding, no border.
pub fn bar<State: 'static, Action: 'static>(
    theme: Theme,
    child: impl WidgetView<State, Action> + 'static,
) -> impl WidgetView<State, Action> {
    sized_box(child)
        .padding(Padding::from(px(theme.padding * 0.6)))
        .background_color(theme.surface)
}

// ── Card (titled bordered container) ──────────────────────────────────

/// A bordered, rounded surface with a bold title, a divider, then the
/// supplied rows. Mirrors `egui_components::Card` minimally.
pub fn card<State: 'static, Action: 'static>(
    theme: Theme,
    card_title: impl Into<String>,
    rows: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action> {
    let body = flex_col(rows)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .gap(px(theme.gap * 0.5));

    let inner = flex_col((
        title::<State, Action>(theme, card_title),
        separator::<State, Action>(theme),
        body,
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(px(theme.gap));

    sized_box(inner)
        .padding(Padding::from(px(theme.padding)))
        .background_color(theme.surface)
        .border(theme.border, px(1.0))
        .corner_radius(px(theme.radius))
}

// ── Rows ──────────────────────────────────────────────────────────────

/// One read-only `label : value` row — muted label on the left, bold
/// value pushed to the right.
pub fn value_row<State: 'static, Action: 'static>(
    theme: Theme,
    row_label: impl Into<String>,
    value: impl Into<String>,
) -> impl WidgetView<State, Action> {
    flex_row((
        muted::<State, Action>(theme, row_label),
        FlexSpacer::Flex(1.0),
        strong::<State, Action>(theme, value),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
}

// ── Buttons ───────────────────────────────────────────────────────────

/// A primary action button.
pub fn button_primary<State: 'static, Action: 'static, F>(
    theme: Theme,
    text_label: impl Into<String>,
    on_click: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let lbl = label(text_label.into())
        .text_size(theme.font_size)
        .color(theme.primary_foreground);
    button(lbl, on_click)
        .padding(Padding::from(px(theme.padding * 0.5)))
        .background_color(theme.primary)
        .corner_radius(px(theme.radius))
}

/// A tab/selection button. Highlights with the accent surface when
/// `selected`, otherwise sits on the plain surface colour.
pub fn tab_button<State: 'static, Action: 'static, F>(
    theme: Theme,
    text_label: impl Into<String>,
    selected: bool,
    on_click: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    let fg = if selected {
        theme.primary_foreground
    } else {
        theme.foreground
    };
    let bg = if selected {
        theme.accent
    } else {
        theme.surface
    };
    let lbl = label(text_label.into())
        .text_size(theme.font_size)
        .color(fg);
    button(lbl, on_click)
        .padding(Padding::from(px(theme.padding * 0.5)))
        .background_color(bg)
        .corner_radius(px(theme.radius))
}

// ── Layout helpers ────────────────────────────────────────────────────

/// Vertical stack of boxed children with the theme gap.
pub fn v_stack<State: 'static, Action: 'static>(
    theme: Theme,
    children: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action> {
    flex_col(children)
        .cross_axis_alignment(CrossAxisAlignment::Stretch)
        .main_axis_alignment(MainAxisAlignment::Start)
        .gap(px(theme.gap))
}

/// Horizontal stack of boxed children with the theme gap.
pub fn h_stack<State: 'static, Action: 'static>(
    theme: Theme,
    children: Vec<Box<AnyWidgetView<State, Action>>>,
) -> impl WidgetView<State, Action> {
    flex_row(children)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .main_axis_alignment(MainAxisAlignment::Start)
        .gap(px(theme.gap))
}
