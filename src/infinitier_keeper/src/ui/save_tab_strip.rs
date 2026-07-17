//! Save-tab strip — one tab per open save game, each with a close (×)
//! button.
//!
//! Sits between the header button bar and the character editor. Clicking
//! a tab switches [`AppState::active_tab`]; clicking its × closes that
//! save. Closing the last remaining tab leaves no save open — the host
//! (`app.rs`) then shows an empty placeholder until another save is
//! loaded via the Load button.
//!
//! Tabs are laid out by hand (measure → greedy row-fit → paint), the
//! same technique the shared `Tabs` widget uses, so a row of tabs wraps
//! onto the next line when it runs out of horizontal space. (A nested
//! `Frame` inside `horizontal_wrapped` won't wrap as a single unit, and
//! `available_width()` is unreliable inside a scroll area — hence the
//! manual layout capped against the clip rect.)

use std::ops::Range;
use std::sync::Arc;

use eframe::egui;
use egui_components::theme::Theme;

use crate::state::AppState;

// Per-chip padding / spacing.
const PAD_L: f32 = 10.0;
const PAD_R: f32 = 8.0;
const GAP_NAME_CLOSE: f32 = 8.0;
const PAD_Y: f32 = 4.0;
const CHIP_GAP_X: f32 = 6.0;
const ROW_GAP_Y: f32 = 4.0;

pub struct SaveTabStrip;

impl SaveTabStrip {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        let theme = Theme::get(ui.ctx());
        egui::Panel::top("keeper_save_tabs")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.background)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                if state.tabs.is_empty() {
                    return;
                }
                let (switch_to, close) = render_tabs(ui, &theme, state);
                if let Some(i) = switch_to {
                    state.active_tab = i;
                }
                if let Some(i) = close {
                    close_tab(state, i);
                }
            });
    }
}

/// A single tab, pre-measured for layout.
struct Chip {
    name: Arc<egui::Galley>,
    close: Arc<egui::Galley>,
    width: f32,
}

/// Measure every tab, pack them into rows that fit the available width,
/// then paint each as a filled chip with an in-chip × close affordance.
/// Returns `(tab_to_select, tab_to_close)`.
fn render_tabs(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
) -> (Option<usize>, Option<usize>) {
    let c = theme.colors;
    let corner = theme.corner();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let row_h = ui.text_style_height(&egui::TextStyle::Body) + PAD_Y * 2.0;

    let chips: Vec<Chip> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let text_color = if i == state.active_tab {
                c.foreground
            } else {
                c.muted_foreground
            };
            let name = ui.painter().layout_no_wrap(
                tab.save_game.display_name().to_string(),
                font.clone(),
                text_color,
            );
            let close = ui.painter().layout_no_wrap(
                "\u{00D7}".to_owned(),
                font.clone(),
                c.muted_foreground,
            );
            let width = PAD_L + name.size().x + GAP_NAME_CLOSE + close.size().x + PAD_R;
            Chip { name, close, width }
        })
        .collect();

    // `available_width()` can stay stuck at a previously-wider content
    // size inside a scroll area; the clip rect is always trimmed to the
    // real viewport, so cap against it (mirrors the `Tabs` widget).
    let visible_w = (ui.clip_rect().right() - ui.cursor().min.x).max(0.0);
    let available_w = ui.available_width().min(visible_w).max(1.0);

    // Greedy line-fit: the first chip on a row always fits (overflowing
    // if it's wider than the row on its own).
    let mut rows: Vec<Range<usize>> = Vec::new();
    let mut start = 0;
    let mut row_w = chips[0].width;
    for (i, chip) in chips.iter().enumerate().skip(1) {
        let next = row_w + CHIP_GAP_X + chip.width;
        if next > available_w {
            rows.push(start..i);
            start = i;
            row_w = chip.width;
        } else {
            row_w = next;
        }
    }
    rows.push(start..chips.len());

    let total_h = row_h * rows.len() as f32 + ROW_GAP_Y * rows.len().saturating_sub(1) as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(available_w, total_h), egui::Sense::hover());
    let painter = ui.painter().clone();
    let base_id = ui.id().with("save_tab");

    let mut switch_to = None;
    let mut close = None;
    let mut y = rect.top();
    for row in &rows {
        let mut x = rect.left();
        for i in row.clone() {
            let chip = &chips[i];
            let selected = i == state.active_tab;
            let chip_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(chip.width, row_h));

            // The × occupies the right of the chip; give its hit area a
            // little extra padding on the left for easier clicking.
            let close_left = chip_rect.right() - PAD_R - chip.close.size().x - GAP_NAME_CLOSE * 0.5;
            let close_rect =
                egui::Rect::from_min_max(egui::pos2(close_left, chip_rect.top()), chip_rect.max);
            let name_rect =
                egui::Rect::from_min_max(chip_rect.min, egui::pos2(close_left, chip_rect.bottom()));

            let name_resp = ui
                .interact(name_rect, base_id.with((i, "name")), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let close_resp = ui
                .interact(close_rect, base_id.with((i, "close")), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Close this save");

            let fill = if selected {
                c.secondary_background
            } else {
                c.muted_background
            };
            painter.rect_filled(chip_rect, corner, fill);
            let name_pos = egui::pos2(
                chip_rect.left() + PAD_L,
                chip_rect.center().y - chip.name.size().y / 2.0,
            );
            painter.galley(name_pos, chip.name.clone(), c.foreground);
            let close_pos = egui::pos2(
                chip_rect.right() - PAD_R - chip.close.size().x,
                chip_rect.center().y - chip.close.size().y / 2.0,
            );
            painter.galley(close_pos, chip.close.clone(), c.muted_foreground);

            if name_resp.clicked() {
                switch_to = Some(i);
            }
            if close_resp.clicked() {
                close = Some(i);
            }
            x += chip.width + CHIP_GAP_X;
        }
        y += row_h + ROW_GAP_Y;
    }
    (switch_to, close)
}

/// Remove tab `i`, keeping [`AppState::active_tab`] pointing at the same
/// logical tab where possible and clamped in range. When the last tab is
/// removed `active_tab` is left at 0 (stale, but the host stops reading
/// the active tab while `tabs` is empty).
fn close_tab(state: &mut AppState, i: usize) {
    if i >= state.tabs.len() {
        return;
    }
    state.tabs.remove(i);
    if i < state.active_tab {
        state.active_tab -= 1;
    }
    state.active_tab = state.active_tab.min(state.tabs.len().saturating_sub(1));
}
