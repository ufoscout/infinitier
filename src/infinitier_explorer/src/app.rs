use eframe::egui;
use infinitier_key_importer::Key;

use crate::components::key_file_tree_view::KeyFileTreeView;
use crate::ui;

pub struct ExplorerApp {
    key_tree: KeyFileTreeView,
}

impl ExplorerApp {
    pub fn new(key: Key) -> Self {
        Self {
            key_tree: KeyFileTreeView::new(&key),
        }
    }
}

impl eframe::App for ExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::left_panel::show(ui, &self.key_tree);
        ui::central_panel::show(ui);
    }
}
