use eframe::egui;

use crate::components::editable_fields::KeeperEditors;
use crate::components::party_selector::PartySelector;
use crate::state::AppState;
use crate::ui::{CharacterPanel, HeaderPanel, SaveAction};

pub struct KeeperApp {
    state: AppState,
    header_panel: HeaderPanel,
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
}

impl KeeperApp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            header_panel: HeaderPanel,
            party_selector: PartySelector::new(),
            character_panel: CharacterPanel,
            editors: KeeperEditors::new(),
            save_action: SaveAction::new(),
        }
    }
}

impl eframe::App for KeeperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Per-frame sync. Both prepare calls are cheap no-ops when
        // nothing changed; the borrows of `&self.state` only overlap
        // the immutable side, so the `&mut self.state` panels take
        // later don't conflict.
        self.party_selector.prepare(&self.state, ui.ctx());
        self.editors.prepare(&self.state);

        if self.header_panel.show(ui, &self.state) {
            self.save_action.open(&self.state);
        }
        self.party_selector.show(ui, &mut self.state);
        self.character_panel
            .show(ui, &mut self.state, &mut self.editors);

        // Modal Save dialog — painted on top of the panels.
        self.save_action.show(ui.ctx(), &self.state);
    }
}
