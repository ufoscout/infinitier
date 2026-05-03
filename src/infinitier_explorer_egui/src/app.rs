use eframe::egui;
use infinitier_core::game;

use crate::state::AppState;
use crate::ui;

pub struct ExplorerApp {
    state: AppState,
    central_panel: ui::central_panel::CentralPanel,
    left_panel: ui::left_panel::LeftPanel,
    bottom_panel: ui::bottom_panel::BottomPanel,
}

impl ExplorerApp {
    pub fn new(game_data: game::GameData) -> Self {
        Self {
            central_panel: ui::central_panel::CentralPanel::new(),
            left_panel: ui::left_panel::LeftPanel::new(&game_data),
            bottom_panel: ui::bottom_panel::BottomPanel,
            state: AppState::new(game_data),
        }
    }
}

impl eframe::App for ExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.bottom_panel.show(ui, &self.state);
        self.left_panel.show(ui, &mut self.state);
        self.central_panel.show(ui, &self.state);
    }
}
