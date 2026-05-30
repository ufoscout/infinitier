//! Self-contained party-member selector component.
//!
//! Owns every piece of state the party rail needs:
//!
//! - the [`PortraitCache`] of decoded BMP textures,
//! - the [`SliderState`] entity for the horizontal selector,
//! - the bookkeeping (`slider_bound_to`, the slider subscription)
//!   that keeps the slider in sync with `KeeperApp::selected_party`.
//!
//! The host wires the selector into `KeeperApp::render` in two steps:
//!
//! 1. Call [`PartySelector::prepare`] once per frame. It lazy-inits
//!    the slider, pushes external selection changes back into it,
//!    and resolves the active portrait into the cache.
//! 2. Embed [`PartySelector::render`] in the layout. It produces the
//!    full left-rail panel (name strip, portrait card, slider).

use std::sync::Arc;

use gpui::{
    AppContext as _, Context, Entity, FontWeight, IntoElement, ObjectFit, ParentElement,
    RenderImage, Styled, StyledImage as _, Subscription, Window, div, img, px,
};
use gpui_component::slider::{Slider, SliderEvent, SliderState, SliderValue};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

mod portraits;

use crate::app::KeeperApp;
use portraits::PortraitCache;

/// Encapsulates portrait caching + slider state + the wiring between
/// them. Lives as a single field on [`KeeperApp`].
#[derive(Default)]
pub struct PartySelector {
    portraits: PortraitCache,
    /// Portrait of the currently-selected member; populated by
    /// [`Self::prepare`] and consumed by [`Self::render`]. We cache
    /// it on `self` rather than threading it through the render
    /// signature so the host can call the two methods independently.
    active_portrait: Option<Arc<RenderImage>>,
    slider: Option<Entity<SliderState>>,
    /// Last slot value we pushed into the slider. Lets us detect
    /// when `selected_party` changes from outside the slider (e.g.
    /// future hotkeys) and mirror that into the slider's value.
    slider_bound_to: Option<usize>,
    _slider_sub: Option<Subscription>,
}

impl PartySelector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Per-frame sync. Call from the top of `KeeperApp::render`.
    ///
    /// Splits the cross-cutting work into three jobs:
    ///
    /// - **Lazy-init the slider** on first call (only when there's
    ///   at least one party member — the widget makes no sense
    ///   otherwise). The subscription routes the user's drag back
    ///   into `KeeperApp::selected_party`.
    /// - **Mirror external selection changes** into the slider's
    ///   value so the thumb moves when something else flipped the
    ///   selection.
    /// - **Resolve the active portrait** into the cache and keep
    ///   the texture on `self` for the renderer to pick up.
    pub fn prepare(
        &mut self,
        party_count: usize,
        selected: Option<usize>,
        selected_cre: Option<&Cre>,
        game_data: &GameData,
        window: &mut Window,
        cx: &mut Context<KeeperApp>,
    ) {
        if self.slider.is_none() && party_count > 0 {
            self.init_slider(selected.unwrap_or(0), party_count, cx);
        }
        if self.slider_bound_to != selected {
            if let (Some(state), Some(idx)) = (&self.slider, selected) {
                let value = idx as f32;
                state.update(cx, |s, cx| s.set_value(value, window, cx));
            }
            self.slider_bound_to = selected;
        }
        self.active_portrait = selected_cre.and_then(|cre| self.portraits.for_cre(cre, game_data));
    }

    /// Build the slider entity + commit subscription. The subscription
    /// closure mutates `KeeperApp` directly — it has to, because
    /// `selected_party` lives on the host, not on the selector.
    fn init_slider(&mut self, initial: usize, count: usize, cx: &mut Context<KeeperApp>) {
        let initial_f = initial as f32;
        let max = count.saturating_sub(1) as f32;
        let state = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(max)
                .step(1.0)
                .default_value(initial_f)
        });
        let sub = cx.subscribe(&state, |this: &mut KeeperApp, _entity, event, cx| {
            let SliderEvent::Change(SliderValue::Single(v)) = event else {
                return;
            };
            let count = this.state.active().imported_gam.party_npcs.len();
            if count == 0 {
                return;
            }
            let idx = (v.round() as i32).clamp(0, count.saturating_sub(1) as i32) as usize;
            this.state.active_mut().selected_party = Some(idx);
            cx.notify();
        });
        self.slider = Some(state);
        self._slider_sub = Some(sub);
        self.slider_bound_to = Some(initial);
    }

    /// Render the left-rail panel: name strip on top, portrait card
    /// in the middle, horizontal slider below. Replaces the old
    /// `ui::party::render` standalone function.
    pub fn render(&self, this: &KeeperApp, cx: &Context<KeeperApp>) -> impl IntoElement {
        let theme = cx.theme();
        let active = this.state.active();
        let party = &active.imported_gam.party_npcs;

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
        let selected = active.selected_party.unwrap_or(0);
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

        // Portrait card.
        col = col.child(portrait_slot(self.active_portrait.clone(), cx));

        // Horizontal slider — only when there's something to slide.
        if count > 1
            && let Some(state) = self.slider.as_ref()
        {
            col = col.child(div().w_full().child(Slider::new(state).horizontal()));
        }

        col
    }
}

/// Picture area, sized to the rail width minus padding. Uses the
/// same absolute-positioning trick the explorer's image viewer uses
/// so taffy can't expand the slot to satisfy the image's intrinsic
/// aspect ratio — `ObjectFit::Contain` centres + scales the image
/// within the fixed slot.
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
