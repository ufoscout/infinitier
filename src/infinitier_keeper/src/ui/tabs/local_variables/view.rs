//! Read-only rendering for the Local Variables tab — EEKeeper's
//! two-column table (Name · Value), in CRE file order.

use eframe::egui;
use egui_components::Label;
use egui_components::scroll_area::ScrollArea;
use infinitier_core::resource::cre::LocalVariable;

pub fn render(ui: &mut egui::Ui, vars: &[LocalVariable]) {
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("local_variables_table")
            .num_columns(2)
            .striped(true)
            .spacing([24.0, 5.0])
            .show(ui, |ui| {
                ui.add(Label::new("Name").strong());
                ui.add(Label::new("Value").strong());
                ui.end_row();

                for var in vars {
                    ui.add(Label::new(var.name.as_str()));
                    ui.add(Label::new(var.value.to_string()));
                    ui.end_row();
                }
            });
    });
}
