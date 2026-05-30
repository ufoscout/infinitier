use eframe::egui;

use crate::components::editable_fields::KeeperEditors;
use crate::components::party_selector::PartySelector;
use crate::state::AppState;
use crate::ui::{CharacterPanel, HeaderPanel, SaveAction, SaveTabStrip};

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

        let header_action = self.header_panel.show(ui);
        if header_action.save_clicked {
            self.save_action.open(&self.state);
        }
        if header_action.load_clicked {
            // Placeholder — the load picker isn't wired yet; the user
            // explicitly flagged more buttons coming, so this branch
            // stays here so the structure is ready for it.
            log::info!("[load] Load button clicked — action not yet implemented");
        }
        // Save-tab strip — one tab per open save. Always painted so
        // the structure is discoverable even with a single tab.
        self.save_tab_strip.show(ui, &mut self.state);
        self.party_selector.show(ui, &mut self.state);
        self.character_panel
            .show(ui, &mut self.state, &mut self.editors);

        // Modal Save dialog — painted on top of the panels.
        self.save_action.show(ui.ctx(), &mut self.state);
    }
}
