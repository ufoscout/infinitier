//! Save-tab strip — one tab per open save game.
//!
//! Sits between the header button bar and the central editor.
//! Clicking a tab switches [`KeeperState::active_tab`]. The strip is
//! always painted, even when only one save is open, so the structure
//! is discoverable as more tabs get added via the Load action
//! (yet-to-be-wired).

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::{ActiveTheme, h_flex};

use crate::app::KeeperApp;

pub fn render(this: &KeeperApp, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    let theme = cx.theme();
    let active_idx = this.state.active_tab;

    let mut row = h_flex()
        .w_full()
        .gap_1()
        .px_3()
        .py_1p5()
        .bg(theme.background)
        .border_b_1()
        .border_color(theme.border)
        .flex_wrap();

    for (idx, tab) in this.state.tabs.iter().enumerate() {
        let selected = idx == active_idx;
        let (bg, fg) = if selected {
            (theme.background, theme.foreground)
        } else {
            (theme.muted, theme.muted_foreground)
        };
        let id = ("save-tab", idx);
        let label = tab.save_name.clone();
        let chip = div()
            .id(id)
            .px_3()
            .py_1()
            .rounded(theme.radius)
            .bg(bg)
            .text_color(fg)
            .border_1()
            .border_color(if selected {
                theme.border
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .hover(|s| s.bg(theme.accent_foreground.opacity(0.05)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.state.active_tab != idx && idx < this.state.tabs.len() {
                    this.state.active_tab = idx;
                    // Force the editors to re-bind to the newly active
                    // tab's CRE values on the next render.
                    this.editors_bound_to = None;
                    cx.notify();
                }
            }))
            .child(label);
        row = row.child(chip);
    }

    // Spacer so the next chip (once Load opens more saves) has room
    // to grow; not strictly required but keeps the right edge of the
    // strip looking intentional.
    row.child(div().min_w(px(0.)).flex_1())
}
