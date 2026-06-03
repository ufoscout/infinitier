//! Read-only rendering for the Memorization tab — EEKeeper's
//! three-column table (Type · Level · Max Can Memorise).

use eframe::egui;
use egui_components::Label;
use egui_components::scroll_area::ScrollArea;

use super::data::MemRow;

pub fn render(ui: &mut egui::Ui, rows: &[MemRow]) {
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("memorization_table")
            .num_columns(3)
            .striped(true)
            .spacing([24.0, 5.0])
            .show(ui, |ui| {
                ui.add(Label::new("Type").strong());
                ui.add(Label::new("Level").strong());
                ui.add(Label::new("Max Can Memorise").strong());
                ui.end_row();

                for row in rows {
                    ui.add(Label::new(row.type_name));
                    ui.add(Label::new(row.level.to_string()));
                    ui.add(Label::new(row.max.to_string()));
                    ui.end_row();
                }
            });
    });
}
