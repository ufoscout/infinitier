//! Read-only rendering for the Local Variables tab — EEKeeper's
//! two-column table (Name · Value), in CRE file order.

use eframe::egui;
use egui_components::{Label, Table, TableColumn};
use infinitier_core::resource::cre::LocalVariable;

/// Initial width of the name column.
const NAME_COL_W: f32 = 300.0;
/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 480.0;

pub fn render(ui: &mut egui::Ui, vars: &[LocalVariable]) {
    let size = egui::vec2(MAX_TABLE_W.min(ui.available_width()), ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("local_variables")
            .striped(true)
            .max_height(ui.available_height())
            .column(
                TableColumn::initial(NAME_COL_W)
                    .at_least(120.0)
                    .clip(true)
                    .header("Name"),
            )
            .column(TableColumn::remainder().header("Value"))
            .show(ui, |body| {
                body.rows(vars.len(), |i, mut row| {
                    let var = &vars[i];
                    row.col(|ui| {
                        ui.add(Label::new(var.name.as_str()));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(var.value.to_string()));
                    });
                });
            });
    });
}
