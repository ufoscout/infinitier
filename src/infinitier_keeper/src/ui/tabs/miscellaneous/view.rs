//! Read-only rendering for the Miscellaneous tab.
//!
//! Mirrors EEKeeper's layout: an "Other" card (turn-undead, tracking,
//! identifier, scripts) on the left, and the "World Time" / "Game
//! Time" / "Joined Party" day-hour-minute cards stacked on the right.
//! Values sit in subtle non-editable framed boxes, like the other
//! read-only tabs.

use eframe::egui;
use egui_components::scroll_area::ScrollArea;
use egui_components::theme::Theme;
use egui_components::{Card, Label, LabelTone};

use infinitier_core::resource::gam::Dhm;

use super::data::MiscData;

/// Width of a value box in the wide "Other" card.
const FIELD_W: f32 = 150.0;
/// Width of a value box in the narrow time cards.
const TIME_W: f32 = 60.0;

pub fn render(ui: &mut egui::Ui, data: &MiscData) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.columns(2, |cols| {
            other_card(&mut cols[0], data);

            time_card(&mut cols[1], "World Time", &data.world_time);
            cols[1].add_space(8.0);
            time_card(&mut cols[1], "Game Time", &data.game_time);
            cols[1].add_space(8.0);
            time_card(&mut cols[1], "Joined Party", &data.joined_party);
        });
    });
}

fn other_card(ui: &mut egui::Ui, data: &MiscData) {
    Card::new().title("Other").divider().show(ui, |ui| {
        egui::Grid::new("misc_other_grid")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                text_row(ui, "Turn Undead", &data.turn_undead.to_string());
                text_row(ui, "Tracking Skill", &data.tracking_skill.to_string());
                text_row(ui, "Tracking Target", &data.tracking_target);
                text_row(ui, "Identifier", &data.identifier.to_string());
                text_row(ui, "Script Name", &data.script_name);
                text_row(ui, "Override Script", &data.override_script);
                text_row(ui, "Class Script", &data.class_script);
                text_row(ui, "Race Script", &data.race_script);
                text_row(ui, "General Script", &data.general_script);
                text_row(ui, "Default Script", &data.default_script);
            });
    });
}

fn time_card(ui: &mut egui::Ui, title: &str, t: &Dhm) {
    Card::new().title(title).divider().show(ui, |ui| {
        egui::Grid::new(format!("misc_time_{title}"))
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                time_row(ui, "Day", t.day);
                time_row(ui, "Hour", t.hour);
                time_row(ui, "Minute", t.minute);
            });
    });
}

// ── Shared widgets ───────────────────────────────────────────────────

fn text_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.add(Label::new(label));
    value_box(ui, value, FIELD_W);
    ui.end_row();
}

fn time_row(ui: &mut egui::Ui, label: &str, value: u32) {
    ui.add(Label::new(label));
    value_box(ui, &value.to_string(), TIME_W);
    ui.end_row();
}

/// Render a value inside a subtle, non-editable framed box so the
/// display reads like EEKeeper's (disabled) field controls.
fn value_box(ui: &mut egui::Ui, text: &str, min_width: f32) {
    let theme = Theme::get(ui.ctx());
    egui::Frame::new()
        .fill(theme.colors.muted_background)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.set_min_width(min_width);
            let tone = if text.is_empty() {
                LabelTone::Muted
            } else {
                LabelTone::Default
            };
            ui.add(Label::new(text).tone(tone));
        });
}
