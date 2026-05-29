//! Root view — implements the `Render` trait that GPUI calls on every
//! frame for the top-of-window entity. Holds the loaded game state +
//! the two pieces of mutable UI state (selected party slot, selected
//! tab) that the panel modules read and the listener closures write.

use gpui::{
    AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div,
};
use gpui_component::slider::{SliderEvent, SliderState, SliderValue};
use gpui_component::{ActiveTheme, Root, h_flex, v_flex};
use infinitier_core::imported_resource::gam::NpcCre;

use crate::editable_fields::KeeperEditors;
use crate::portraits::PortraitCache;
use crate::state::KeeperState;
use crate::ui::tabs::CharacterTab;
use crate::ui::{character, header, party};

pub struct KeeperApp {
    pub state: KeeperState,
    pub selected_party: Option<usize>,
    pub selected_tab: CharacterTab,
    /// Text-input scaffolding for every editable row on the
    /// abilities tab. Created lazily on the first `render` because
    /// building an `InputState` needs `&mut Window`, which
    /// `KeeperApp::new` doesn't have. Once created, the input states
    /// + their commit subscriptions live for the app's lifetime.
    pub editors: Option<KeeperEditors>,
    /// Slot whose CRE values were last pushed into `editors`. The
    /// abilities tab compares it to `selected_party` and re-binds
    /// the inputs when the user switches characters (or after a
    /// commit, to echo the clamped value back into the input).
    pub editors_bound_to: Option<usize>,
    /// Decoded BMP textures for the party portraits. Populated
    /// lazily during render — the loader reads `PORTRTn.bmp` from
    /// the save folder (path lives on `KeeperState`).
    pub portraits: PortraitCache,
    /// Slider state for stepping through the party. Lazy-init on
    /// first render alongside the editors. The party-rail widget
    /// renders a [`gpui_component::slider::Slider`] bound to it.
    pub party_slider: Option<Entity<SliderState>>,
    /// Last slot the slider was set to from app → slider. Lets us
    /// detect external selection changes (e.g. via future hotkeys)
    /// and push them back into the slider's value.
    pub party_slider_bound_to: Option<usize>,
    _party_slider_sub: Option<Subscription>,
}

impl KeeperApp {
    pub fn new(state: KeeperState) -> Self {
        let selected_party = if state.imported_gam.party_npcs.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            state,
            selected_party,
            selected_tab: CharacterTab::Abilities,
            editors: None,
            editors_bound_to: None,
            portraits: PortraitCache::default(),
            party_slider: None,
            party_slider_bound_to: None,
            _party_slider_sub: None,
        }
    }
}

impl Render for KeeperApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Lazy-init the field editors on first render. The
        // InputState constructor needs `&mut Window`, which the
        // `KeeperApp::new` call site (the `cx.new(|_| …)` closure
        // inside `main`) doesn't have, so we build them here.
        if self.editors.is_none() {
            self.editors = Some(KeeperEditors::new(window, cx));
        }
        // Push the active CRE / GAM values into the inputs whenever
        // the user switches party slots or after a commit clamp.
        if self.editors_bound_to != self.selected_party {
            if let Some(idx) = self.selected_party
                && let Some(npc) = self.state.imported_gam.party_npcs.get(idx)
                && let Some(NpcCre::Cre(cre)) = npc.cre.as_ref()
                && let Some(editors) = self.editors.as_ref()
            {
                editors.rebind_to(cre, &self.state.imported_gam, window, cx);
            }
            self.editors_bound_to = self.selected_party;
        }
        // Lazy-init the party slider. SliderState needs the party
        // count (max index = count-1) so it must run after `state`
        // is populated. The subscription routes value changes back
        // into `selected_party`.
        let party_count = self.state.imported_gam.party_npcs.len();
        if self.party_slider.is_none() && party_count > 0 {
            let initial = self.selected_party.unwrap_or(0) as f32;
            let max = (party_count.saturating_sub(1)) as f32;
            let state = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(max)
                    .step(1.0)
                    .default_value(initial)
            });
            let sub = cx.subscribe(&state, move |this, _entity, event, cx| {
                let SliderEvent::Change(SliderValue::Single(v)) = event else {
                    return;
                };
                let count = this.state.imported_gam.party_npcs.len();
                if count == 0 {
                    return;
                }
                let idx = (v.round() as i32).clamp(0, count.saturating_sub(1) as i32) as usize;
                this.selected_party = Some(idx);
                cx.notify();
            });
            self.party_slider = Some(state);
            self._party_slider_sub = Some(sub);
            self.party_slider_bound_to = self.selected_party;
        }
        // Push the slider's value when `selected_party` changes via
        // any other path (none right now, but the rebind is cheap).
        if self.party_slider_bound_to != self.selected_party {
            if let (Some(state), Some(idx)) = (&self.party_slider, self.selected_party) {
                let value = idx as f32;
                state.update(cx, |s, cx| s.set_value(value, window, cx));
            }
            self.party_slider_bound_to = self.selected_party;
        }

        // Resolve the selected member's portrait *before* building
        // the render tree — the cache lookup needs `&mut self` to
        // populate on miss, which conflicts with the `&self` borrow
        // the render-tree builders need below. Lookup mirrors
        // NearInfinity: take the resref from the CRE header, then
        // resolve it through `GameData` (override → BIFs) with a
        // fallback to `<root>/portraits/<name>.bmp` for custom
        // imports.
        let selected_portrait = self.selected_party.and_then(|idx| {
            let npc = self.state.imported_gam.party_npcs.get(idx)?;
            let cre = match npc.cre.as_ref()? {
                NpcCre::Cre(boxed) => boxed.as_ref(),
                NpcCre::Ref(_) => return None,
            };
            self.portraits.for_cre(cre, &self.state.game_data)
        });

        // gpui-component's `Root::render` only paints `self.view`;
        // dialog / sheet / notification overlays have to be embedded
        // by the host view, otherwise `window.open_dialog(...)` would
        // populate `active_dialogs` but never paint anything.
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("keeper-root")
            .size_full()
            .relative()
            .child(
                v_flex()
                    .size_full()
                    .bg(cx.theme().background)
                    .text_color(cx.theme().foreground)
                    .child(header::render(self, cx))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .child(party::render(self, selected_portrait, cx))
                            .child(div().w_px().bg(cx.theme().border))
                            .child(character::render(self, cx)),
                    ),
            )
            .children(dialog_layer)
            .children(sheet_layer)
            .children(notification_layer)
    }
}
