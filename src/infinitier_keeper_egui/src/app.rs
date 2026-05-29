use eframe::egui;

use crate::components::party_selector::PartySelector;
use crate::state::AppState;
use crate::ui::{CharacterPanel, HeaderPanel};

pub struct KeeperApp {
    state: AppState,
    header_panel: HeaderPanel,
    party_selector: PartySelector,
    character_panel: CharacterPanel,
}

impl KeeperApp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            header_panel: HeaderPanel,
            party_selector: PartySelector::new(),
            character_panel: CharacterPanel,
        }
    }
}

impl eframe::App for KeeperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Resolve the active portrait + mirror `selected_party_index`
        // into the slider value *before* any panel renders — the
        // lookup borrows `&self.state` immutably and would conflict
        // with the `&mut self.state` the panels take below.
        self.party_selector.prepare(&self.state, ui.ctx());

        self.header_panel.show(ui, &self.state);
        self.party_selector.show(ui, &mut self.state);
        self.character_panel.show(ui, &mut self.state);
    }
}
