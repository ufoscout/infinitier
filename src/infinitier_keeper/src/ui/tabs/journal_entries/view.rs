//! Read-only rendering for the Journal Entries tab.
//!
//! A four-column table (Journal Type / Journal Entry / Chapter / Time).
//! The "Journal Entry" cell shows the entry's *full* text, wrapped to the
//! column width, and each row is sized to fit all of it — done by
//! measuring every entry's wrapped height and feeding it to the table's
//! [`heterogeneous_rows`](egui_components::TableBodyUi::heterogeneous_rows)
//! (the uniform-height `rows` can't grow a row to its content).
//!
//! Entry text comes from `dialog.tlk`. Resolving a strref means
//! re-parsing the whole TLK, so we load it once and memoise every
//! strref → text lookup in the egui frame store; subsequent repaints
//! cost nothing.

use std::collections::HashMap;

use eframe::egui;

use egui_components::{Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::resource::gam::GameTicks;

use super::calendar::Calendar;
use super::data::JournalRow;

/// egui frame-store key for the strref → text cache.
const TEXT_CACHE: &str = "journal_text_cache";
/// egui frame-store key for the cached calendar.
const CALENDAR_CACHE: &str = "journal_calendar";

/// Fixed column widths (the Journal Entry column takes the rest).
const TYPE_W: f32 = 120.0;
const CHAPTER_W: f32 = 60.0;
const TIME_W: f32 = 200.0;
/// Min / max width for the wrapping Journal Entry column.
const MIN_ENTRY_W: f32 = 240.0;
const MAX_ENTRY_W: f32 = 720.0;
/// Room left for the vertical scrollbar so the columns never overflow
/// (an egui_extras table has no horizontal scrollbar).
const SCROLLBAR_W: f32 = 16.0;
/// Inset subtracted from the entry width when measuring wrap height —
/// conservative (slightly over-tall) so the text is never clipped.
const ENTRY_TEXT_INSET: f32 = 28.0;
/// Extra vertical padding added to each measured row height.
const ROW_VPAD: f32 = 8.0;

pub fn render(ui: &mut egui::Ui, rows: &[JournalRow], game_data: &GameData) {
    if rows.is_empty() {
        ui.add(Label::new("This save has no journal entries."));
        return;
    }

    let texts = resolve_texts(ui, game_data, rows);
    let calendar = resolve_calendar(ui, game_data);

    // The entry column takes the width left over after the fixed columns,
    // clamped so it is neither too narrow nor uncomfortably wide.
    let entry_w = (ui.available_width() - TYPE_W - CHAPTER_W - TIME_W - SCROLLBAR_W)
        .clamp(MIN_ENTRY_W, MAX_ENTRY_W);
    let table_w = TYPE_W + entry_w + CHAPTER_W + TIME_W + SCROLLBAR_W;

    // Measure each entry's wrapped height at the entry column's text width,
    // using the same body font the cell renders with, so every row grows
    // to show its whole text.
    let font = egui::TextStyle::Body.resolve(ui.style());
    let wrap_w = (entry_w - ENTRY_TEXT_INSET).max(40.0);
    let min_row_h = ui.spacing().interact_size.y;
    let heights: Vec<f32> = rows
        .iter()
        .map(|r| {
            let text = texts.get(&r.strref).map(String::as_str).unwrap_or("");
            let text_h = ui
                .painter()
                .layout(text.to_owned(), font.clone(), egui::Color32::WHITE, wrap_w)
                .size()
                .y;
            (text_h + ROW_VPAD).max(min_row_h)
        })
        .collect();

    let size = egui::vec2(table_w, ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("journal_entries")
            .striped(true)
            .max_height(ui.available_height())
            .column(TableColumn::exact(TYPE_W).clip(true).header("Journal Type"))
            .column(TableColumn::exact(entry_w).header("Journal Entry"))
            .column(TableColumn::exact(CHAPTER_W).header("Chapter"))
            .column(TableColumn::exact(TIME_W).clip(true).header("Time"))
            .show(ui, |body| {
                body.heterogeneous_rows(heights.into_iter(), |i, mut row| {
                    let r = &rows[i];
                    let text = texts.get(&r.strref).map(String::as_str).unwrap_or("");
                    row.col(|ui| {
                        ui.add(egui::Label::new(r.type_label));
                    });
                    // The full entry text, wrapped — the row was sized to it.
                    row.col(|ui| {
                        ui.add(egui::Label::new(text).wrap());
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(r.chapter.to_string()));
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(time_text(calendar.as_ref(), r.time)));
                    });
                });
            });
    });
}

/// Format an in-game timestamp the way the engine's journal does. When
/// the calendar resources are available this is the full
/// `Day N, Hour H (DD Month, Year)`; otherwise it falls back to a plain
/// day / hour / minute clock.
fn time_text(calendar: Option<&Calendar>, time: GameTicks) -> String {
    if let Some(cal) = calendar {
        return cal.format(time);
    }
    let dhm = time.dhm();
    format!("Day {}, {:02}:{:02}", dhm.day, dhm.hour, dhm.minute)
}

/// Resolve every entry's strref to its `dialog.tlk` text, memoising the
/// results in the egui frame store. The TLK is parsed at most once: we
/// load it only when there are uncached strrefs to look up.
fn resolve_texts(
    ui: &mut egui::Ui,
    game_data: &GameData,
    rows: &[JournalRow],
) -> HashMap<u32, String> {
    let id = egui::Id::new(TEXT_CACHE);
    let cached = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<u32, String>>(id))
        .unwrap_or_default();

    let misses: Vec<u32> = rows
        .iter()
        .map(|r| r.strref)
        .filter(|s| !cached.contains_key(s))
        .collect();
    if misses.is_empty() {
        return cached;
    }

    let tlk = game_data.dialog_tlk().ok();
    ui.ctx().data_mut(|d| {
        let map = d.get_temp_mut_or_default::<HashMap<u32, String>>(id);
        for strref in misses {
            map.entry(strref)
                .or_insert_with(|| tlk.as_ref().and_then(|t| t.get(strref)).unwrap_or_default());
        }
        map.clone()
    })
}

/// Build the Harptos calendar once and memoise it in the egui frame
/// store. Returns `None` (every frame) if the calendar resources are
/// missing, in which case the caller falls back to a plain clock.
fn resolve_calendar(ui: &mut egui::Ui, game_data: &GameData) -> Option<Calendar> {
    let id = egui::Id::new(CALENDAR_CACHE);
    if let Some(hit) = ui.ctx().data_mut(|d| d.get_temp::<Calendar>(id)) {
        return Some(hit);
    }
    let calendar = Calendar::load(game_data)?;
    ui.ctx().data_mut(|d| d.insert_temp(id, calendar.clone()));
    Some(calendar)
}
