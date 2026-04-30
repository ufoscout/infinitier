use eframe::egui;

pub fn show(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.centered_and_justified(|ui| {
            ui.label("Select a resource from the panel on the left.");
        });
    });
}
