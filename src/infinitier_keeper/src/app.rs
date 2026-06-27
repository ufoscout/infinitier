use eframe::egui;

use crate::components::editable_fields::KeeperEditors;
use crate::components::party_selector::PartySelector;
use crate::state::AppState;
use crate::ui::{CharacterPanel, HeaderPanel, ItemBrowser, LoadAction, SaveAction, SaveTabStrip};

pub struct KeeperApp {
    state: AppState,
    header_panel: HeaderPanel,
    save_tab_strip: SaveTabStrip,
    party_selector: PartySelector,
    character_panel: CharacterPanel,
    /// In-flight text buffers for every editable row on the
    /// abilities tab + the Attacks dropdown index. Mirrors the GPUI
    /// keeper's `KeeperEditors`, just without InputState entities —
    /// egui's immediate-mode model collapses the rebind + commit
    /// plumbing to plain owned `String`s held in a map.
    editors: KeeperEditors,
    /// Save-button + confirmation-dialog state. Shown when the user
    /// clicks the header's Save button.
    save_action: SaveAction,
    /// Load-button picker: the "Open Single Player Saved Game" modal.
    load_action: LoadAction,
    /// The floating Item Browser window (all its state + rendering).
    item_browser: ItemBrowser,
    /// Whether the floating "Items" browser window is open. Toggled by
    /// the header's Items button; the window's own close button clears it.
    items_window_open: bool,
    /// Whether the floating "Spells" browser window is open. Toggled by
    /// the header's Spells button; the window's own close button clears it.
    spells_window_open: bool,
}

impl KeeperApp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            header_panel: HeaderPanel,
            save_tab_strip: SaveTabStrip,
            party_selector: PartySelector::new(),
            character_panel: CharacterPanel,
            editors: KeeperEditors::new(),
            save_action: SaveAction::new(),
            load_action: LoadAction::new(),
            item_browser: ItemBrowser::new(),
            items_window_open: false,
            spells_window_open: false,
        }
    }
}

impl KeeperApp {
    /// Paint the movable / resizable / closable "Items" and "Spells"
    /// browser windows when their header toggles are on. The Items window
    /// is a full browser; the Spells window is still a placeholder.
    fn show_tool_windows(&mut self, ctx: &egui::Context) {
        self.item_browser
            .show(ctx, &mut self.items_window_open, &self.state.game_data);
        egui::Window::new("Spells")
            .open(&mut self.spells_window_open)
            .default_size([420.0, 320.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("Spell browser — coming soon.");
            });
    }
}

impl eframe::App for KeeperApp {
    /// Pre-frame hook (runs once before [`Self::ui`], inside the egui
    /// pass). Works around egui issue https://github.com/emilk/egui/issues/2142: a focused text field only
    /// surrenders keyboard focus on Escape / Tab / arrow-nav or when
    /// another *focusable* widget is clicked — clicking empty space or a
    /// plain label leaves it focused forever, so `Response::lost_focus()`
    /// never fires and the commit-on-blur path in
    /// [`KeeperEditors::show_input`] never runs.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        surrender_focus_on_outside_click(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let has_save = !self.state.tabs.is_empty();
        let header_action = self.header_panel.show(ui);
        if header_action.save_clicked && has_save {
            self.save_action.open(&self.state);
        }
        if header_action.load_clicked {
            self.load_action.open(&mut self.state);
        }
        // Header's Items / Spells buttons toggle their floating windows.
        if header_action.items_clicked {
            self.items_window_open = !self.items_window_open;
        }
        if header_action.spells_clicked {
            self.spells_window_open = !self.spells_window_open;
        }
        self.show_tool_windows(ui.ctx());
        // ── Update phase — everything that can mutate `state` runs
        // first, so the view below reads a settled state in the same
        // frame (no repaint round-trip). The tab strip switches/closes
        // tabs; the modal dialogs (foreground layer, so still painted
        // on top) may open a different save into the active tab.
        self.save_tab_strip.show(ui, &mut self.state);
        self.save_action.show(ui.ctx(), &mut self.state);
        self.load_action.show(ui.ctx(), &mut self.state);

        // ── View phase — render from the now-settled state. `prepare`
        // re-mirrors the editor buffers from whatever save is active, so
        // a switch made above shows immediately.
        if self.state.tabs.is_empty() {
            empty_state(ui);
        } else {
            self.party_selector.prepare(&self.state, ui.ctx());
            self.editors.prepare(&self.state);
            self.party_selector.show(ui, &mut self.state);
            self.character_panel
                .show(ui, &mut self.state, &mut self.editors);
        }
    }
}

/// Placeholder shown when every save tab has been closed, inviting the
/// user to open one via the header's Load button.
fn empty_state(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("No save loaded");
            ui.add_space(6.0);
            ui.label("Use the Load button above to open a saved game.");
        });
    });
}

/// Drop keyboard focus from the currently-focused widget when the latest
/// pointer press landed outside it. See [`KeeperApp::logic`] for why this
/// is needed (egui https://github.com/emilk/egui/issues/2142) and why it has to run at the top of the frame.
///
/// Generic on purpose: any focused widget surrenders focus on an outside
/// click, which matches normal desktop behaviour and means every editable
/// row — present and future — gets correct commit-on-blur for free,
/// without each call site having to opt in.
fn surrender_focus_on_outside_click(ctx: &egui::Context) {
    // Only a fresh pointer press can move focus off a field.
    if !ctx.input(|i| i.pointer.any_pressed()) {
        return;
    }
    let Some(focused) = ctx.memory(|m| m.focused()) else {
        return;
    };
    let Some(press_pos) = ctx.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    // Rect of the focused widget as laid out last frame — it sits in the
    // same place this frame, since `logic` runs before anything is
    // redrawn. A press inside it means the user is interacting with the
    // field itself (clicking to place the caret, selecting text), so leave
    // focus alone; only an outside press counts as a blur.
    let clicked_inside = ctx
        .read_response(focused)
        .is_some_and(|r| r.interact_rect.contains(press_pos));
    if !clicked_inside {
        ctx.memory_mut(|m| m.surrender_focus(focused));
    }
}
