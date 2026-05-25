use eframe::egui;

use crate::state::AppState;
use crate::ui::{AbilitiesPanel, HeaderPanel, PartyPanel};

pub struct KeeperApp {
    state: AppState,
    header_panel: HeaderPanel,
    party_panel: PartyPanel,
    abilities_panel: AbilitiesPanel,
}

impl KeeperApp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            header_panel: HeaderPanel,
            party_panel: PartyPanel,
            abilities_panel: AbilitiesPanel,
        }
    }
}

impl eframe::App for KeeperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.header_panel.show(ui, &self.state);
        self.party_panel.show(ui, &mut self.state);
        self.abilities_panel.show(ui, &self.state);
    }
}
