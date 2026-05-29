//! Root view — implements the `Render` trait that GPUI calls on every
//! frame for the top-of-window entity. Holds the loaded game state +
//! the two pieces of mutable UI state (selected party slot, selected
//! tab) that the panel modules read and the listener closures write.

use gpui::{Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, Root, h_flex, v_flex};
use infinitier_core::imported_resource::gam::NpcCre;

use crate::components::editable_fields::KeeperEditors;
use crate::components::party_selector::PartySelector;
use crate::state::KeeperState;
use crate::ui::tabs::CharacterTab;
use crate::ui::{character, header};

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
    /// Owns the portrait cache, the party-slider state, and all the
    /// per-frame sync between them and `selected_party`. The left
    /// rail is built by [`PartySelector::render`].
    pub party_selector: PartySelector,
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
            party_selector: PartySelector::new(),
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

        // Hand the party selector everything it needs to refresh
        // itself for this frame — slider sync + portrait lookup. The
        // borrow has to happen here (not inside the render-tree
        // builders) because the selector mutates its cache.
        let selected_cre = self.selected_party.and_then(|idx| {
            let npc = self.state.imported_gam.party_npcs.get(idx)?;
            match npc.cre.as_ref()? {
                NpcCre::Cre(boxed) => Some(boxed.as_ref()),
                NpcCre::Ref(_) => None,
            }
        });
        let party_count = self.state.imported_gam.party_npcs.len();
        self.party_selector.prepare(
            party_count,
            self.selected_party,
            selected_cre,
            &self.state.game_data,
            window,
            cx,
        );

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
                            .child(self.party_selector.render(self, cx))
                            .child(div().w_px().bg(cx.theme().border))
                            .child(character::render(self, cx)),
                    ),
            )
            .children(dialog_layer)
            .children(sheet_layer)
            .children(notification_layer)
    }
}
