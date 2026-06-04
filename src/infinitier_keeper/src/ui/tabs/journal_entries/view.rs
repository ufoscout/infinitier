//! Read-only rendering for the Journal Entries tab.
//!
//! Mirrors EEKeeper's layout: a master table (Journal Type / Journal
//! Entry / Chapter / Time) over a detail pane that shows the full text
//! of the selected entry. The table only paints the first line of each
//! entry; the rest is reserved for the pane below.
//!
//! Entry text comes from `dialog.tlk`. Resolving a strref means
//! re-parsing the whole TLK, so we load it once and memoise every
//! strref → text lookup in the egui frame store; subsequent repaints
//! cost nothing.

use std::collections::HashMap;

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use egui_components::Label;
use infinitier_core::game::GameData;
use infinitier_core::resource::gam::GameTicks;

use super::calendar::Calendar;
use super::data::JournalRow;

/// egui frame-store key for the strref → text cache.
const TEXT_CACHE: &str = "journal_text_cache";
/// egui frame-store key for the cached calendar.
const CALENDAR_CACHE: &str = "journal_calendar";
/// egui frame-store key for the selected row index.
const SELECTED: &str = "journal_selected_row";

pub fn render(ui: &mut egui::Ui, rows: &[JournalRow], game_data: &GameData) {
    if rows.is_empty() {
        ui.add(Label::new("This save has no journal entries."));
        return;
    }

    let texts = resolve_texts(ui, game_data, rows);
    let calendar = resolve_calendar(ui, game_data);
    let row_h = egui::TextStyle::Body.resolve(ui.style()).size + 8.0;

    let sel_id = egui::Id::new(SELECTED);
    let mut selected = ui
        .ctx()
        .data_mut(|d| d.get_temp::<usize>(sel_id))
        .unwrap_or(0);
    if selected >= rows.len() {
        selected = 0;
    }

    // Reserve the lower slice of the tab for the detail pane and give
    // the master table everything above it.
    let full_h = ui.available_height();
    let detail_h = (full_h * 0.3).clamp(90.0, 220.0);
    let table_h = (full_h - detail_h - 8.0).max(60.0);

    let mut clicked: Option<usize> = None;
    ui.allocate_ui(egui::vec2(ui.available_width(), table_h), |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .sense(egui::Sense::click())
            .column(Column::initial(130.0).resizable(true))
            .column(Column::remainder().clip(true))
            .column(Column::initial(70.0).resizable(true))
            .column(Column::initial(190.0).clip(true).resizable(true))
            .header(row_h, |mut header| {
                header.col(|ui| {
                    ui.add(Label::new("Journal Type").strong());
                });
                header.col(|ui| {
                    ui.add(Label::new("Journal Entry").strong());
                });
                header.col(|ui| {
                    ui.add(Label::new("Chapter").strong());
                });
                header.col(|ui| {
                    ui.add(Label::new("Time").strong());
                });
            })
            .body(|body| {
                body.rows(row_h, rows.len(), |mut row| {
                    let i = row.index();
                    row.set_selected(i == selected);
                    let r = &rows[i];
                    let first_line = texts
                        .get(&r.strref)
                        .and_then(|t| t.lines().find(|l| !l.trim().is_empty()))
                        .unwrap_or("");
                    row.col(|ui| {
                        ui.add(Label::new(r.type_label));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(first_line));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(r.chapter.to_string()));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(time_text(calendar.as_ref(), r.time)));
                    });
                    if row.response().clicked() {
                        clicked = Some(i);
                    }
                });
            });
    });

    if let Some(i) = clicked {
        selected = i;
        ui.ctx().data_mut(|d| d.insert_temp(sel_id, i));
    }

    ui.separator();

    let detail = rows
        .get(selected)
        .and_then(|r| texts.get(&r.strref))
        .map(String::as_str)
        .unwrap_or("");
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(Label::new(detail));
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
