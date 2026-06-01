//! Read-only rendering for the Characteristics tab.
//!
//! Mirrors EEKeeper's layout: a left-hand identity column (gender,
//! race, alignment, class, …) and a right-hand stack of two cards —
//! "Kill Stats" and "Miscellaneous". Editing controls (the per-row
//! "Set Value" buttons and dropdowns) are intentionally omitted; this
//! pass is display-only.

use std::collections::HashMap;

use eframe::egui;
use egui_components::scroll_area::ScrollArea;
use egui_components::theme::Theme;
use egui_components::{Card, Checkbox, Label, LabelTone};
use infinitier_core::game::GameData;

use super::data::{CharData, MISC_FLAGS};

const FIELD_MIN_W: f32 = 190.0;

/// Paint the whole tab: identity column on the left, Kill Stats +
/// Miscellaneous cards on the right.
pub fn render(ui: &mut egui::Ui, data: &CharData, game_data: &GameData) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.columns(2, |cols| {
            identity_column(&mut cols[0], data);

            kill_stats_card(&mut cols[1], data, game_data);
            cols[1].add_space(8.0);
            miscellaneous_card(&mut cols[1], data);
        });
    });
}

// ── Left: identity ───────────────────────────────────────────────────

fn identity_column(ui: &mut egui::Ui, data: &CharData) {
    egui::Grid::new("characteristics_identity")
        .num_columns(2)
        .spacing([10.0, 7.0])
        .show(ui, |ui| {
            simple_row(ui, "Gender", &data.gender);
            simple_row(ui, "Race", &data.race);
            simple_row(ui, "Alignment", &data.alignment);

            // Class carries an inline "Fallen" (paladin/ranger) flag.
            label_cell(ui, "Class");
            ui.horizontal(|ui| {
                read_only_check(ui, "Fallen", data.fallen);
                value_box(ui, &data.class);
            });
            ui.end_row();

            // Original Class carries the inline "Dual Class" flag.
            label_cell(ui, "Original Class");
            ui.horizontal(|ui| {
                value_box(ui, &data.original_class);
                read_only_check(ui, "Dual Class", data.dual_class);
            });
            ui.end_row();

            simple_row(ui, "Kit", &data.kit);
            simple_row(ui, "Racial", &data.racial_enemy);
            simple_row(ui, "Enemy/Ally", &data.enemy_ally);
            simple_row(ui, "State", &data.state);

            label_cell(ui, "Movement");
            ui.horizontal(|ui| {
                value_box(ui, &data.movement.to_string());
                ui.add(Label::new("Zero is normal speed.").tone(LabelTone::Muted));
            });
            ui.end_row();
        });
}

fn simple_row(ui: &mut egui::Ui, label: &str, value: &str) {
    label_cell(ui, label);
    value_box(ui, value);
    ui.end_row();
}

// ── Right: Kill Stats ────────────────────────────────────────────────

fn kill_stats_card(ui: &mut egui::Ui, data: &CharData, game_data: &GameData) {
    Card::new()
        .title("Kill Stats")
        .divider()
        .show(ui, |ui| {
            let k = &data.kill;
            let strongest = resolve_strref(ui, game_data, k.strongest_name_strref);
            egui::Grid::new("characteristics_kill_stats")
                .num_columns(2)
                .spacing([10.0, 7.0])
                .show(ui, |ui| {
                    stat_row(ui, "Strongest Kill Name", &strongest);
                    stat_row(ui, "Strongest Kill XP", &k.strongest_xp.to_string());
                    stat_row(ui, "Chapter Kills", &k.chapter_kills.to_string());
                    stat_row(ui, "Chapter Kills XP", &k.chapter_kills_xp.to_string());
                    stat_row(ui, "Game Kills", &k.game_kills.to_string());
                    stat_row(ui, "Game Kills XP", &k.game_kills_xp.to_string());
                });
        });
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.add_sized([130.0, 20.0], left_label(label));
    value_box(ui, value);
    ui.end_row();
}

// ── Right: Miscellaneous flags ───────────────────────────────────────

fn miscellaneous_card(ui: &mut egui::Ui, data: &CharData) {
    Card::new()
        .title("Miscellaneous")
        .divider()
        .show(ui, |ui| {
            // EEKeeper lays the flags out in two columns: the first
            // half of MISC_FLAGS down the left, the rest down the right.
            let mid = MISC_FLAGS.len().div_ceil(2);
            ui.columns(2, |cols| {
                for (i, (label, bit)) in MISC_FLAGS.iter().enumerate() {
                    let col = if i < mid { &mut cols[0] } else { &mut cols[1] };
                    read_only_check(col, label, data.flags & bit != 0);
                    col.add_space(3.0);
                }
            });
        });
}

// ── Shared widgets ───────────────────────────────────────────────────

/// A left-aligned grid label cell.
fn label_cell(ui: &mut egui::Ui, text: &str) {
    ui.add(left_label(text));
}

fn left_label(text: &str) -> Label {
    Label::new(text)
}

/// Render a value inside a subtle, non-editable framed box so the
/// display reads like EEKeeper's (disabled) field controls.
fn value_box(ui: &mut egui::Ui, text: &str) {
    let theme = Theme::get(ui.ctx());
    egui::Frame::new()
        .fill(theme.colors.muted_background)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.set_min_width(FIELD_MIN_W);
            let tone = if text.is_empty() {
                LabelTone::Muted
            } else {
                LabelTone::Default
            };
            ui.add(Label::new(text).tone(tone));
        });
}

/// A disabled checkbox reflecting a read-only boolean flag.
fn read_only_check(ui: &mut egui::Ui, label: &str, checked: bool) {
    let mut state = checked;
    ui.add(Checkbox::new(&mut state, label).disabled(true));
}

/// Resolve a TLK strref to its string, memoising results in the egui
/// frame store so we parse `dialog.tlk` at most once per distinct
/// strref instead of on every repaint.
fn resolve_strref(ui: &mut egui::Ui, game_data: &GameData, strref: u32) -> String {
    if strref == 0 || strref == 0xFFFF_FFFF {
        return String::new();
    }
    let id = egui::Id::new("characteristics_strref_cache");
    if let Some(hit) = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<u32, String>>(id))
        .and_then(|m| m.get(&strref).cloned())
    {
        return hit;
    }
    let resolved = game_data
        .dialog_tlk()
        .ok()
        .and_then(|tlk| tlk.get(strref))
        .unwrap_or_default();
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<HashMap<u32, String>>(id)
            .insert(strref, resolved.clone());
    });
    resolved
}
