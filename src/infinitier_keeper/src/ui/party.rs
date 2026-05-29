//! Left rail — party-member selector. A single card shows the
//! currently-selected member's portrait with a name label on top
//! and a horizontal slider below to flip through the party.

use std::sync::Arc;

use gpui::{
    Context, FontWeight, IntoElement, ObjectFit, ParentElement, RenderImage, StyledImage as _,
    div, img, px,
};
use gpui::Styled;
use gpui_component::slider::Slider;
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::app::KeeperApp;

pub fn render(
    this: &KeeperApp,
    portrait: Option<Arc<RenderImage>>,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    let theme = cx.theme();
    let party = &this.state.imported_gam.party_npcs;

    let mut col = v_flex()
        .w(px(240.))
        .h_full()
        .px_3()
        .py_3()
        .gap_3()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.sidebar_border);

    if party.is_empty() {
        return col
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .child("Party"),
            )
            .child(div().h_px().bg(theme.sidebar_border))
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child("No party members in this save."),
            );
    }

    let count = party.len();
    let selected = this.selected_party.unwrap_or(0);
    let member = &party[selected];

    // Top strip: bold name on the left, slot counter on the right.
    let name = if member.display_name.is_empty() {
        format!("Slot {}", selected + 1)
    } else {
        member.display_name.clone()
    };
    col = col.child(
        h_flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(14.))
                    .child(name),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(format!("{} / {}", selected + 1, count)),
            ),
    );

    // Middle: portrait card.
    col = col.child(portrait_slot(portrait, cx));

    // Bottom: horizontal slider. Shown only when more than one
    // party member exists — otherwise there's nothing to slide.
    if count > 1
        && let Some(state) = this.party_slider.as_ref()
    {
        col = col.child(div().w_full().child(Slider::new(state).horizontal()));
    }

    col
}

/// Picture area, sized to the rail width minus padding. We use the
/// same absolute-positioning trick the explorer's image viewer uses
/// so taffy can't expand the slot to satisfy the image's intrinsic
/// aspect ratio — `ObjectFit::Contain` then centres + scales the
/// picture within the fixed slot.
fn portrait_slot(
    portrait: Option<Arc<RenderImage>>,
    cx: &Context<KeeperApp>,
) -> impl IntoElement {
    let theme = cx.theme();
    let slot = div()
        .w_full()
        .h(px(280.))
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted)
        .relative()
        .overflow_hidden();

    match portrait {
        Some(image) => slot.child(
            img(image)
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .size_full()
                .object_fit(ObjectFit::Contain),
        ),
        None => slot.child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child("No portrait"),
        ),
    }
}
