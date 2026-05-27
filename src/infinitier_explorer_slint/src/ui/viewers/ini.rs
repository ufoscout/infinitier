//! INI viewer — flattens `[section] key=value` into a 3-column table.
//! The egui original only showed an "INI Viewer" label.

use infinitier_core::resource::ini::Ini;

use crate::{MainWindow, TableRow};

const MAX_ROWS: usize = 2000;

pub fn populate(window: &MainWindow, ini: Ini) {
    let headers: Vec<slint::SharedString> = vec![
        "Section".into(),
        "Key".into(),
        "Value".into(),
    ];

    let mut flat_rows: Vec<TableRow> = Vec::new();
    for section in &ini.sections {
        for entry in &section.entries {
            if flat_rows.len() >= MAX_ROWS {
                break;
            }
            let cells: Vec<slint::SharedString> = vec![
                section.name.as_str().into(),
                entry.key.as_str().into(),
                entry.value.as_str().into(),
            ];
            flat_rows.push(TableRow {
                cells: slint::ModelRc::new(slint::VecModel::from(cells)),
            });
        }
        if flat_rows.len() >= MAX_ROWS {
            break;
        }
    }
    let total: usize = ini.sections.iter().map(|s| s.entries.len()).sum();
    let truncated = total > MAX_ROWS;

    let subtitle = if truncated {
        format!(
            "{} sections · {} entries · showing first {MAX_ROWS}",
            ini.sections.len(),
            total,
        )
    } else {
        format!(
            "{} sections · {} entries",
            ini.sections.len(),
            total,
        )
    };

    window.set_viewer_kind("table".into());
    window.set_table_headers(slint::ModelRc::new(slint::VecModel::from(headers)));
    window.set_table_rows(slint::ModelRc::new(slint::VecModel::from(flat_rows)));
    window.set_table_title("INI".into());
    window.set_table_subtitle(subtitle.into());
}
