use eframe::egui;

use crate::state::AppState;
use crate::ui::{CharacterPanel, HeaderPanel, PartyPanel};

pub struct KeeperApp {
    state: AppState,
    header_panel: HeaderPanel,
    party_panel: PartyPanel,
    character_panel: CharacterPanel,
}

impl KeeperApp {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            header_panel: HeaderPanel,
            party_panel: PartyPanel,
            character_panel: CharacterPanel,
        }
    }
}

impl eframe::App for KeeperApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.header_panel.show(ui, &self.state);
        self.party_panel.show(ui, &mut self.state);
        self.character_panel.show(ui, &mut self.state);
    }
}
