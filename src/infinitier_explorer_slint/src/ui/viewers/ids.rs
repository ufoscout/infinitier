//! IDS viewer — renders the value/name pairs as a 3-column table.
//! The egui original only showed an "IDS Viewer" label.

use infinitier_core::resource::ids::Ids;

use crate::{MainWindow, TableRow};

const MAX_ROWS: usize = 2000;

pub fn populate(window: &MainWindow, ids: Ids) {
    let headers: Vec<slint::SharedString> = vec![
        "Value".into(),
        "Value (raw)".into(),
        "Name".into(),
    ];

    let total = ids.entries.len();
    let truncated = total > MAX_ROWS;

    let rows: Vec<TableRow> = ids
        .entries
        .iter()
        .take(MAX_ROWS)
        .map(|e| {
            let cells: Vec<slint::SharedString> = vec![
                e.value.to_string().into(),
                e.value_str.as_str().into(),
                e.name.as_str().into(),
            ];
            TableRow {
                cells: slint::ModelRc::new(slint::VecModel::from(cells)),
            }
        })
        .collect();

    let subtitle = if truncated {
        format!("{total} entries · showing first {MAX_ROWS}")
    } else {
        format!("{total} entries")
    };

    window.set_viewer_kind("table".into());
    window.set_table_headers(slint::ModelRc::new(slint::VecModel::from(headers)));
    window.set_table_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    window.set_table_title("IDS".into());
    window.set_table_subtitle(subtitle.into());
}
