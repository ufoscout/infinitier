//! Read-only rendering for the Proficiencies tab — EEKeeper's
//! three-column table (Proficiency · First Class · Second Class).

use eframe::egui;
use egui_components::scroll_area::ScrollArea;
use egui_components::{Label, LabelTone};

use super::data::ProfRow;

const VALUE_COL_W: f32 = 90.0;

pub fn render(ui: &mut egui::Ui, rows: &[ProfRow]) {
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("proficiencies_table")
            .num_columns(3)
            .striped(true)
            .spacing([24.0, 5.0])
            .show(ui, |ui| {
                ui.add(Label::new("Proficiency").strong());
                ui.add(Label::new("First Class").strong());
                ui.add(Label::new("Second Class").strong());
                ui.end_row();

                for row in rows {
                    ui.add(Label::new(row.name.clone()));
                    value_cell(ui, row.first);
                    value_cell(ui, row.second);
                    ui.end_row();
                }
            });
    });
}

/// A proficiency-point cell: the number, or blank for zero (matching
/// EEKeeper, which leaves untrained proficiencies empty).
fn value_cell(ui: &mut egui::Ui, points: u32) {
    let text = if points == 0 {
        String::new()
    } else {
        points.to_string()
    };
    ui.add_sized(
        [VALUE_COL_W, ui.spacing().interact_size.y],
        Label::new(text).tone(LabelTone::Default),
    );
}
