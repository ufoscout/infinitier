//! Top-of-window metadata strip. A muted band carrying a theme-toggle
//! pill and four caption / value columns (Game, Folder, Save, GAM).

use eframe::egui;
use egui_components::{Button, Label, LabelTone, Size, Tooltip};
use egui_components::theme::{Theme, ThemeMode};

use crate::state::AppState;

pub struct HeaderPanel;

impl HeaderPanel {
    /// Returns `true` when the user clicked the Save button this
    /// frame — the host opens the save dialog in response.
    pub fn show(&self, ui: &mut egui::Ui, state: &AppState) -> bool {
        let theme = Theme::get(ui.ctx());
        let mut save_clicked = false;
        egui::Panel::top("keeper_header")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.muted_background)
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    theme_toggle_button(ui);
                    ui.add_space(16.0);
                    field(ui, "Game", &format!("{:?}", state.game_data.game()));
                    ui.add_space(24.0);
                    field(
                        ui,
                        "Folder",
                        &state
                            .game_data
                            .fs()
                            .get_roots()
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    ui.add_space(24.0);
                    field(ui, "Save", &state.save_name);
                    ui.add_space(24.0);
                    field(ui, "GAM", &format!("{:?}", state.save.version));
                    // Push the Save button to the far right of the
                    // header strip.
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.add(Button::primary("Save").small()).clicked() {
                                save_clicked = true;
                            }
                        },
                    );
                });
            });
        save_clicked
    }
}

/// Small pill on the left of the header that swaps the active theme.
/// Label shows the *target* mode (matches the GPUI keeper convention).
fn theme_toggle_button(ui: &mut egui::Ui) {
    let theme = Theme::get(ui.ctx());
    let target_label = match theme.mode {
        ThemeMode::Dark => "Light",
        ThemeMode::Light => "Dark",
    };
    if ui.add(Button::ghost(target_label).small()).clicked() {
        let next = match theme.mode {
            ThemeMode::Dark => Theme::light(),
            ThemeMode::Light => Theme::dark(),
        };
        next.install(ui.ctx());
    }
}

/// One header column: muted caption above a bold value. The value
/// is single-line + ellipsis-truncated and clamped to
/// [`FIELD_MAX_WIDTH`] so a long folder list can't blow up the strip
/// vertically and push the rest of the keeper below the viewport.
/// The full text stays accessible via the hover tooltip.
fn field(ui: &mut egui::Ui, caption: &str, value: &str) {
    // Skip the cell entirely when the strip has no room left — keeps
    // the inner `set_max_width` from receiving a negative value
    // (egui panics on that, and a window can be shrunk that small).
    let avail = ui.available_width();
    if avail < FIELD_MIN_RENDER_WIDTH {
        return;
    }
    let width = avail.min(FIELD_MAX_WIDTH);
    let theme = Theme::get(ui.ctx());
    // `ui.vertical` (same as the original layout) so the column
    // anchors with the other cells inside the parent
    // `horizontal(left_to_right(Center))` row. The cap goes on the
    // child ui via `set_max_width` so the truncate-aware value
    // label knows how much room it has.
    ui.vertical(|ui| {
        ui.set_max_width(width);
        ui.add(Label::new(caption).tone(LabelTone::Muted).size(Size::Small));
        // `egui_components::Label` doesn't expose a wrap-mode
        // setter, so mirror its style on a `RichText` and feed that
        // to the egui primitive (which supports `.truncate()`).
        let rich = egui::RichText::new(value)
            .strong()
            .color(theme.colors.foreground)
            .font(egui::FontId::proportional(theme.metrics.font_size_md));
        let response = ui.add(egui::Label::new(rich).truncate());
        Tooltip::new(value).attach(response);
    });
}

/// Per-field width cap. Picked so the four columns + theme button +
/// Save button comfortably share a ~1400 px window, and so the
/// strip stays single-line at the keeper's typical workspace size.
const FIELD_MAX_WIDTH: f32 = 240.0;
/// Don't try to paint a column when the leftover space drops below
/// this — there's no useful render at narrower widths, and the
/// inner truncation math goes negative.
const FIELD_MIN_RENDER_WIDTH: f32 = 60.0;
