//! 2DA viewer — renders the parsed table as a grid. The egui original
//! only showed a "2DA Viewer" label; this port surfaces the actual
//! data so the resource is useful.

use infinitier_core::resource::two_da::TwoDA;

use crate::{MainWindow, TableRow};

const MAX_ROWS: usize = 1000;

pub fn populate(window: &MainWindow, twoda: TwoDA) {
    let mut headers: Vec<slint::SharedString> =
        vec!["#".into()];
    headers.extend(twoda.headers.iter().map(|h| h.as_str().into()));

    // Sort row keys so the table renders deterministically — `TwoDA`
    // stores rows in a `HashMap`.
    let mut keys: Vec<&String> = twoda.rows.keys().collect();
    keys.sort();
    let total = keys.len();
    let truncated = total > MAX_ROWS;
    keys.truncate(MAX_ROWS);

    let rows: Vec<TableRow> = keys
        .into_iter()
        .map(|k| {
            let mut cells: Vec<slint::SharedString> = vec![k.as_str().into()];
            cells.extend(
                twoda.rows[k]
                    .iter()
                    .map(|c| c.as_str().into()),
            );
            TableRow {
                cells: slint::ModelRc::new(slint::VecModel::from(cells)),
            }
        })
        .collect();

    let subtitle = if truncated {
        format!(
            "{} rows × {} cols · default \"{}\" · showing first {MAX_ROWS}",
            total,
            twoda.headers.len(),
            twoda.default,
        )
    } else {
        format!(
            "{} rows × {} cols · default \"{}\"",
            total,
            twoda.headers.len(),
            twoda.default,
        )
    };

    window.set_viewer_kind("table".into());
    window.set_table_headers(slint::ModelRc::new(slint::VecModel::from(headers)));
    window.set_table_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    window.set_table_title("2DA".into());
    window.set_table_subtitle(subtitle.into());
}
