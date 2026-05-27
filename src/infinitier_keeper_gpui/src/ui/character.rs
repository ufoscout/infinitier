//! Central character panel — title row, tab strip, then the active
//! tab's body. Mirrors the Slint `CharacterPanel` layout.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use infinitier_core::imported_resource::gam::NpcCre;

use crate::app::KeeperApp;
use crate::ui::tabs::{self, CharacterTab};

pub fn render(this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    // Snapshot the few theme colours we paint statically. We can't
    // hold `cx.theme()` across the calls to `render_tab_strip` /
    // `render_body` (both need `&mut cx`) so copy the Hsla values out
    // up front — `Hsla` is `Copy`.
    let theme = cx.theme();
    let secondary = theme.secondary;
    let border = theme.border;

    let title = match this.selected_party.and_then(|i| this.state.imported_gam.party_npcs.get(i)) {
        Some(m) if !m.display_name.is_empty() => {
            format!("{}. {}", m.index + 1, m.display_name)
        }
        Some(m) => format!("Party slot {}", m.index + 1),
        None => "No party member selected".to_string(),
    };

    let body: AnyElement = render_body(this, cx).into_any_element();

    v_flex()
        .flex_1()
        .h_full()
        .min_w_0()
        .child(
            div()
                .px_4()
                .py_2()
                .text_size(px(16.))
                .font_weight(FontWeight::BOLD)
                .bg(secondary)
                .border_b_1()
                .border_color(border)
                .child(title),
        )
        .child(render_tab_strip(this, cx))
        .child(div().h_px().bg(border))
        .child(
            div()
                .id("tab-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .child(body),
        )
}

/// Tab strip — horizontal `flex_wrap` row of chip-style buttons, one
/// per `CharacterTab` variant. Selected gets the `accent` colour;
/// others get `secondary`.
fn render_tab_strip(this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    let theme = cx.theme();
    let mut row = h_flex()
        .flex_wrap()
        .gap_1p5()
        .px_3()
        .py_2()
        .bg(theme.secondary);
    for tab in CharacterTab::ALL {
        let selected = this.selected_tab == *tab;
        let (bg, fg) = if selected {
            (theme.accent, theme.accent_foreground)
        } else {
            (theme.background, theme.foreground)
        };
        let id = ("tab-chip", *tab as usize);
        let label = tab.label();
        let t = *tab;
        row = row.child(
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded(theme.radius)
                .bg(bg)
                .text_color(fg)
                .cursor_pointer()
                .hover(|s| s.bg(theme.accent_foreground.opacity(0.05)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected_tab = t;
                    cx.notify();
                }))
                .child(label),
        );
    }
    row
}

/// Pick the right tab module + handle the empty / external-CRE / empty-slot
/// fallbacks so each tab body sees a real `Cre`.
fn render_body(this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    let Some(member) = this
        .selected_party
        .and_then(|i| this.state.imported_gam.party_npcs.get(i))
    else {
        return tabs::stub::render("Pick a party member on the left.").into_any_element();
    };

    match &member.cre {
        Some(NpcCre::Cre(cre)) => tabs::dispatch(this.selected_tab, cre, &this.state.imported_gam, cx)
            .into_any_element(),
        Some(NpcCre::Ref(resref)) => tabs::stub::render(format!(
            "External CRE '{resref}' — embedded record not present in this GAM.",
        ))
        .into_any_element(),
        None => tabs::stub::render("Empty party slot — no creature record to edit.")
            .into_any_element(),
    }
}
