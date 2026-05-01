use eframe::egui;
use infinitier_core::game;

use crate::components::key_file_tree_view::KeyFileTreeView;
use crate::state::AppState;
use crate::ui;

pub struct ExplorerApp {
    key_tree: KeyFileTreeView,
    state: AppState,
}

impl ExplorerApp {
    pub fn new(game_data: game::GameData) -> Self {
        Self {
            key_tree: KeyFileTreeView::new(&game_data),
            state: AppState::new(game_data),
        }
    }
}

impl eframe::App for ExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::bottom_panel::show(ui, &self.state);
        ui::left_panel::show(ui, &self.key_tree, &mut self.state);
        ui::central_panel::show(ui);
    }
}
