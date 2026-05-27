//! Left rail — list of party-NPC slots. Selected row gets the
//! `accent` background, matching the Slint spike.

use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{ActiveTheme, v_flex};

use crate::app::KeeperApp;

pub fn render(this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    let theme = cx.theme();

    let mut col = v_flex()
        .w(px(240.))
        .h_full()
        .px_3()
        .py_3()
        .gap_2()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.sidebar_border)
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .child("Party"),
        )
        .child(div().h_px().bg(theme.sidebar_border));

    if this.state.imported_gam.party_npcs.is_empty() {
        col = col.child(div().text_color(theme.muted_foreground).child("No party members in this save."));
        return col;
    }

    let selected = this.selected_party;
    let mut list = v_flex().gap_1();
    for (i, member) in this.state.imported_gam.party_npcs.iter().enumerate() {
        let label = if member.display_name.is_empty() {
            format!("Slot {}", i + 1)
        } else {
            format!("{}. {}", i + 1, member.display_name)
        };
        let is_selected = selected == Some(i);
        let (bg, fg) = if is_selected {
            (theme.accent, theme.accent_foreground)
        } else {
            (theme.transparent, theme.sidebar_foreground)
        };
        list = list.child(
            div()
                .id(("party-row", i))
                .px_2()
                .py_1()
                .rounded(theme.radius)
                .bg(bg)
                .text_color(fg)
                .cursor_pointer()
                .hover(|s| s.bg(theme.sidebar_accent))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected_party = Some(i);
                    cx.notify();
                }))
                .child(label),
        );
    }
    col.child(list)
}
