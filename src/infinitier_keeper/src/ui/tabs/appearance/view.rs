//! Read-only rendering for the Appearance tab.
//!
//! Mirrors EEKeeper: an "Appearance" row showing the creature's
//! animation as a name (e.g. "Fighter Male Human"), then a "Colors"
//! card laying out the seven colour swatches in a two-column grid.
//!
//! The animation name comes from ANIMATE.IDS and the swatches from the
//! palette BMP; both are parsed once and memoised in the egui frame
//! store (the resolved name per id, the palette once, each swatch
//! texture per colour index).

use std::collections::HashMap;

use eframe::egui;
use egui_components::theme::Theme;
use egui_components::{Card, Label, LabelTone};

use infinitier_core::game::GameData;

use super::data::AppearanceData;
use super::palette::{Palette, SWATCH_PX};

/// egui frame-store key for the animation-id → name cache.
const NAME_CACHE: &str = "appearance_name_cache";
/// egui frame-store key for the cached palette.
const PALETTE_CACHE: &str = "appearance_palette";
/// egui frame-store key for the colour-index → swatch-texture cache.
const SWATCH_CACHE: &str = "appearance_swatch_cache";

pub fn render(ui: &mut egui::Ui, data: &AppearanceData, game_data: &GameData) {
    let name = resolve_name(ui, game_data, data.animation_id);
    let palette = resolve_palette(ui, game_data);
    let swatches = resolve_swatches(ui, palette.as_ref(), data);

    // "Appearance" row.
    ui.horizontal(|ui| {
        ui.add(Label::new("Appearance"));
        value_box(ui, &name, 220.0);
    });
    ui.add_space(8.0);

    // "Colors" card: two columns of (label, swatch) pairs.
    Card::new().title("Colors").divider().show(ui, |ui| {
        egui::Grid::new("appearance_colors_grid")
            .num_columns(4)
            .spacing([14.0, 10.0])
            .show(ui, |ui| {
                for pair in data.colors.chunks(2) {
                    for slot in pair {
                        ui.add(Label::new(slot.label));
                        swatch_image(ui, swatches.get(&slot.index));
                    }
                    if pair.len() == 1 {
                        // Pad the trailing odd cell (Metal) so the grid
                        // stays aligned.
                        ui.label("");
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
    });
}

/// Paint a single swatch (or an empty placeholder if it didn't resolve).
fn swatch_image(ui: &mut egui::Ui, texture: Option<&egui::TextureHandle>) {
    let side = SWATCH_PX as f32;
    match texture {
        Some(tex) => {
            ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(side, side)));
        }
        None => {
            ui.allocate_space(egui::vec2(side, side));
        }
    }
}

/// Resolve an animation id to a prettified name via ANIMATE.IDS,
/// memoised per id. Falls back to the raw hex id when unresolved.
fn resolve_name(ui: &mut egui::Ui, game_data: &GameData, animation_id: u32) -> String {
    let id = egui::Id::new(NAME_CACHE);
    if let Some(hit) = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<u32, String>>(id))
        .and_then(|m| m.get(&animation_id).cloned())
    {
        return hit;
    }
    let resolved = game_data
        .import_ids_by_name("ANIMATE")
        .ok()
        .and_then(|ids| ids.of_value(animation_id as i32).map(title_case))
        .unwrap_or_else(|| format!("0x{animation_id:04X}"));
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<HashMap<u32, String>>(id)
            .insert(animation_id, resolved.clone());
    });
    resolved
}

/// Build the palette once and memoise it in the egui frame store.
fn resolve_palette(ui: &mut egui::Ui, game_data: &GameData) -> Option<Palette> {
    let id = egui::Id::new(PALETTE_CACHE);
    if let Some(hit) = ui.ctx().data_mut(|d| d.get_temp::<Palette>(id)) {
        return Some(hit);
    }
    let palette = Palette::load(game_data)?;
    ui.ctx().data_mut(|d| d.insert_temp(id, palette.clone()));
    Some(palette)
}

/// Build (and cache) a swatch texture per distinct colour index used by
/// this creature.
fn resolve_swatches(
    ui: &mut egui::Ui,
    palette: Option<&Palette>,
    data: &AppearanceData,
) -> HashMap<u8, egui::TextureHandle> {
    let id = egui::Id::new(SWATCH_CACHE);
    let mut cache: HashMap<u8, egui::TextureHandle> =
        ui.ctx().data_mut(|d| d.get_temp(id)).unwrap_or_default();

    let Some(palette) = palette else {
        return cache;
    };
    let misses: Vec<u8> = data
        .colors
        .iter()
        .map(|c| c.index)
        .filter(|i| !cache.contains_key(i))
        .collect();
    if misses.is_empty() {
        return cache;
    }

    for index in misses {
        let image = palette.swatch(index);
        let tex = ui.ctx().load_texture(
            format!("appearance-swatch/{index}"),
            image,
            egui::TextureOptions::NEAREST,
        );
        cache.insert(index, tex);
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, cache.clone()));
    cache
}

/// `FIGHTER_MALE_HUMAN` → `Fighter Male Human`.
fn title_case(symbol: &str) -> String {
    symbol
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render a value inside a subtle, non-editable framed box, matching the
/// other read-only tabs.
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
