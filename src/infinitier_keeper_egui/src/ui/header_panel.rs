//! Top-of-window metadata strip. Matches the Slint keeper's
//! `HeaderPanel`: a row of caption / value columns over an
//! `alternate_background` band, with the bolder values up top.

use eframe::egui;
use infinitier_egui_common::theme;

use crate::state::AppState;

pub struct HeaderPanel;

impl HeaderPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &AppState) {
        let palette = theme::active();
        let muted_fg = mix(palette.foreground, palette.alternate_background, 0.4);

        egui::Panel::top("keeper_header")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(palette.alternate_background)
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    theme_toggle_button(ui);
                    ui.add_space(16.0);
                    field(
                        ui,
                        muted_fg,
                        "Game",
                        &format!("{:?}", state.game_data.game()),
                    );
                    ui.add_space(24.0);
                    field(
                        ui,
                        muted_fg,
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
                    field(ui, muted_fg, "Save", &state.save_name);
                    ui.add_space(24.0);
                    field(ui, muted_fg, "GAM", &format!("{:?}", state.save.version));
                });
            });
    }
}

/// Small button at the leftmost end of the header that cycles between
/// the two palettes in `infinitier_egui_common::theme`. Label shows
/// the mode the click would switch *to*, matching the keeper_gpui
/// pattern.
fn theme_toggle_button(ui: &mut egui::Ui) {
    let palette = theme::active();
    let label = if palette.dark_mode { "Light" } else { "Dark" };
    if ui.small_button(label).clicked() {
        let next = if palette.dark_mode {
            &theme::LIGHT
        } else {
            &theme::DARK
        };
        theme::apply(ui.ctx(), next);
    }
}

/// One header column: muted caption above a bold value, matching the
/// Slint `HeaderField` component.
fn field(ui: &mut egui::Ui, caption_color: egui::Color32, caption: &str, value: &str) {
    ui.vertical(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(caption)
                    .size(11.0)
                    .color(caption_color),
            )
            .truncate(),
        );
        ui.add(egui::Label::new(egui::RichText::new(value).strong().size(14.0)).truncate());
    });
}

/// Local lerp — same as the helper in `egui_common::theme` but kept
/// inline so the panel can run without a public re-export.
fn mix(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let blend = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    let [ar, ag, ab, _] = a.to_array();
    let [br, bg, bb, _] = b.to_array();
    egui::Color32::from_rgb(blend(ar, br), blend(ag, bg), blend(ab, bb))
}
