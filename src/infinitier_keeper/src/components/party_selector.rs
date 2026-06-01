//! Self-contained party-member selector — egui port of the GPUI
//! keeper's `PartySelector`.
//!
//! Owns the [`PortraitCache`] and an internal slider value. The host
//! wires it into the keeper layout with two calls:
//!
//! 1. [`PartySelector::prepare`] once per frame — resolves the active
//!    portrait and syncs the slider value to the latest
//!    `selected_party_index`.
//! 2. [`PartySelector::show`] to render the left-rail panel (name
//!    strip + portrait card + horizontal slider).
//!
//! egui is immediate-mode so there's no subscription bookkeeping —
//! `show` reads / writes `state.selected_party_index` directly.

use eframe::egui;
use egui_components::theme::Theme;
use egui_components::{Avatar, Card, Label, LabelTone, Size, Slider};
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::Cre;

use crate::components::portraits::PortraitCache;
use crate::state::AppState;

/// Width of the left rail. Matches the GPUI keeper's `w(px(240.))`,
/// loosened slightly so the slot counter never wraps.
const RAIL_WIDTH: f32 = 240.0;
/// Aspect (width / height) we fall back to when no portrait is
/// loaded — derived from BG2EE / BGEE / IWDEE large portraits which
/// are 169×256. Vanilla BG1/BG2 large portraits are 110×170; both
/// land near 0.66, so a single default is fine for the placeholder.
const PORTRAIT_FALLBACK_ASPECT: f32 = 169.0 / 256.0;
/// Safety cap so a freak portrait (e.g. a square sprite) can't grow
/// the rail vertically past the keeper's viewport. The slot's true
/// height is `slot_width / aspect`, clamped against this.
const PORTRAIT_MAX_HEIGHT: f32 = 420.0;

#[derive(Default)]
pub struct PartySelector {
    portraits: PortraitCache,
    /// Resolved portrait of the currently-selected member, refreshed
    /// in `prepare`. Stored here so `show` (which gets `&self`)
    /// doesn't have to re-run the cache lookup.
    active_portrait: Option<egui::TextureHandle>,
    /// Slider's internal `f32` value. Mirrors
    /// `AppState::selected_party_index` — `prepare` reads the host
    /// state, `show` writes any drag back via the rounded f32.
    slider_value: f32,
}

impl PartySelector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Per-frame sync. Resolves the active portrait into the cache
    /// and pulls the slider value from `state.selected_party_index`.
    /// Call once at the top of the host's frame.
    pub fn prepare(&mut self, state: &AppState, ctx: &egui::Context) {
        self.slider_value = state.active().selected_party_index.unwrap_or(0) as f32;
        self.active_portrait =
            selected_cre(state).and_then(|cre| self.portraits.for_cre(cre, &state.game_data, ctx));
    }

    /// Render the left rail. Mutates `state.selected_party_index`
    /// when the user drags the slider.
    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut AppState) {
        let theme = Theme::get(ui.ctx());
        egui::Panel::left("keeper_party")
            .resizable(true)
            .default_size(RAIL_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.muted_background)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show_inside(ui, |ui| {
                let party_count = state.active().save.party_npcs.len();
                if party_count == 0 {
                    self.show_empty(ui);
                    return;
                }
                let selected = state.active().selected_party_index.unwrap_or(0);
                self.show_header(ui, state, selected);
                ui.add_space(8.0);
                self.show_portrait(ui, state, selected);
                if party_count > 1 {
                    ui.add_space(8.0);
                    self.show_slider(ui, state, party_count);
                }
            });
    }

    fn show_empty(&self, ui: &mut egui::Ui) {
        Card::new().title("Party").divider().show(ui, |ui| {
            ui.add(Label::new("No party members in this save.").tone(LabelTone::Muted));
        });
    }

    /// Bold name on the left, "selected+1 / count" counter on the
    /// right — same strip the GPUI keeper paints.
    fn show_header(&self, ui: &mut egui::Ui, state: &AppState, selected: usize) {
        let count = state.active().save.party_npcs.len();
        let name = display_name(state, selected);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 0.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add(Label::new(&name).strong().size(Size::Large));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        Label::new(format!("{} / {}", selected + 1, count))
                            .tone(LabelTone::Muted)
                            .size(Size::Small),
                    );
                });
            },
        );
    }

    /// Portrait card whose **proportions follow the loaded large
    /// portrait's own dimensions**. The slot consumes the full rail
    /// width and computes its height as `width / aspect`, where the
    /// aspect is read from the active [`egui::TextureHandle`] (or
    /// [`PORTRAIT_FALLBACK_ASPECT`] when no portrait is loaded). The
    /// rail can be resized freely — the slot stays in the same
    /// proportional shape as the actual BMP, so the image fits
    /// edge-to-edge without cropping. egui's `Image::corner_radius`
    /// keeps the painted texture inside the slot's rounded border.
    ///
    /// Without a portrait the slot shows an initials [`Avatar`].
    fn show_portrait(&self, ui: &mut egui::Ui, state: &AppState, selected: usize) {
        let theme = Theme::get(ui.ctx());
        let avail_w = ui.available_width();
        // Use the loaded portrait's native aspect ratio so resizes
        // keep the picture's proportions. Fall back to the BG2EE
        // 169:256 ratio when no portrait is resolved yet.
        let aspect = self
            .active_portrait
            .as_ref()
            .map(|tex| {
                let s = tex.size_vec2();
                if s.x > 0.0 && s.y > 0.0 {
                    s.x / s.y
                } else {
                    PORTRAIT_FALLBACK_ASPECT
                }
            })
            .unwrap_or(PORTRAIT_FALLBACK_ASPECT);
        let slot_height = (avail_w / aspect).min(PORTRAIT_MAX_HEIGHT);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(avail_w, slot_height), egui::Sense::hover());
        // Background fill + border. Painted first; the image
        // (drawn at the same rect with matching corner_radius)
        // covers everything inside the rounded shape.
        {
            let painter = ui.painter_at(rect);
            painter.rect(
                rect,
                theme.corner(),
                theme.colors.muted_background,
                theme.border_stroke(),
                egui::StrokeKind::Inside,
            );
        }

        match &self.active_portrait {
            Some(texture) => {
                let tex_size = texture.size_vec2();
                if tex_size.x > 0.0 && tex_size.y > 0.0 {
                    egui::Image::from_texture(texture)
                        .uv(cover_fit_uv(rect.size(), tex_size))
                        .corner_radius(theme.corner())
                        .paint_at(ui, rect);
                }
            }
            None => {
                // Centre an initials Avatar inside the slot.
                let avatar_size = (rect.width() * 0.55).min(rect.height() * 0.55);
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                ));
                child.add(Avatar::from_name(display_name(state, selected)).size(avatar_size));
            }
        }
    }

    fn show_slider(&mut self, ui: &mut egui::Ui, state: &mut AppState, count: usize) {
        // The egui-components `Slider` paints its track with
        // `muted_background`, which is also the rail's panel fill —
        // the track would otherwise vanish against the surrounding
        // sidebar. Drop the slider inside a bordered `background`
        // frame so the track contrasts cleanly.
        let theme = Theme::get(ui.ctx());
        let max = (count.saturating_sub(1)) as f32;
        egui::Frame::new()
            .fill(theme.colors.background)
            .stroke(theme.border_stroke())
            .corner_radius(theme.corner())
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let response =
                    ui.add(Slider::new(&mut self.slider_value, 0.0..=max).width(avail_w));
                if response.changed() {
                    let idx = (self.slider_value.round() as i64).clamp(0, max as i64) as usize;
                    state.active_mut().selected_party_index = Some(idx);
                    // Snap the local mirror so subsequent drag deltas
                    // see the rounded value rather than the raw float.
                    self.slider_value = idx as f32;
                }
            });
    }
}

fn selected_cre(state: &AppState) -> Option<&Cre> {
    let active = state.active();
    let idx = active.selected_party_index?;
    let npc = active.save.party_npcs.get(idx)?;
    match npc.cre.as_ref()? {
        NpcCre::Cre(boxed) => Some(boxed.as_ref()),
        NpcCre::Ref(_) => None,
    }
}

fn display_name(state: &AppState, idx: usize) -> String {
    let member = &state.active().save.party_npcs[idx];
    if member.display_name.is_empty() {
        format!("Slot {}", idx + 1)
    } else {
        member.display_name.clone()
    }
}

/// UV rect that centre-crops a texture so it fully covers `slot_size`
/// (the CSS `object-fit: cover` equivalent). Whichever axis "wins"
/// the aspect-ratio race fills the slot; the other axis's excess is
/// trimmed evenly off both sides via the returned UV window.
fn cover_fit_uv(slot_size: egui::Vec2, tex_size: egui::Vec2) -> egui::Rect {
    let aspect_slot = slot_size.x / slot_size.y;
    let aspect_tex = tex_size.x / tex_size.y;
    if aspect_slot > aspect_tex {
        // Slot is wider relative to texture — fill width, crop top + bottom.
        let crop_v = aspect_tex / aspect_slot;
        let v_pad = (1.0 - crop_v) / 2.0;
        egui::Rect::from_min_max(egui::pos2(0.0, v_pad), egui::pos2(1.0, 1.0 - v_pad))
    } else {
        // Slot is taller relative to texture — fill height, crop left + right.
        let crop_u = aspect_slot / aspect_tex;
        let u_pad = (1.0 - crop_u) / 2.0;
        egui::Rect::from_min_max(egui::pos2(u_pad, 0.0), egui::pos2(1.0 - u_pad, 1.0))
    }
}
