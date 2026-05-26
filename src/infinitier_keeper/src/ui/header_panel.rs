use eframe::egui;

use crate::state::AppState;

pub struct HeaderPanel;

impl HeaderPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &AppState) {
        egui::Panel::top("keeper_header")
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Game:");
                    ui.label(format!("{:?}", state.game_data.game()));
                    ui.separator();
                    ui.strong("Game folder:");
                    ui.label(
                        state
                            .game_data.fs().get_roots()
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Save:");
                    ui.label(&state.save.name);
                    ui.separator();
                    ui.strong("Version:");
                    ui.label(format!("{:?}", state.save.gam.version));
                });
            });
    }
}
