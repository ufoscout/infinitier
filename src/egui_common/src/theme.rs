//! Slint-Fluent-compatible theme for egui.
//!
//! Two palettes — [`DARK`] and [`LIGHT`] — that mirror the colours
//! Slint's built-in `Palette` global emits when its `color-scheme` is
//! set to `Dark` or `Light` (i.e. the Fluent style, which is the
//! Slint default). Apply one via [`apply`]; pick the matching card
//! fill via [`card_frame`].
//!
//! The Slint Fluent palette only exposes a small set of "intent"
//! colours (background, accent, etc.). egui's `Visuals` requires
//! distinct hover/active fills for interactive widgets, and Slint
//! doesn't surface those — we synthesise reasonable variants a
//! couple of steps brighter (dark) or darker (light) than
//! `alternate_background`, picked to match the brightness ramp the
//! Slint Fluent buttons render at in practice.

use std::sync::RwLock;

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle, Visuals};

/// Subset of Slint's built-in `Palette` global, transcribed to egui
/// colours. Field order matches Slint's `FluentPalette` for easy
/// cross-referencing against
/// `i-slint-compiler/widgets/fluent/styling.slint`.
#[derive(Clone, Debug)]
pub struct Palette {
    pub background: Color32,
    pub foreground: Color32,
    pub alternate_background: Color32,
    pub alternate_foreground: Color32,
    pub control_background: Color32,
    pub control_foreground: Color32,
    pub accent_background: Color32,
    pub accent_foreground: Color32,
    pub selection_background: Color32,
    pub selection_foreground: Color32,
    pub border: Color32,
    /// `widgets.hovered.bg_fill` in the applied `Visuals`. Synthetic —
    /// see the module docs.
    pub hover: Color32,
    /// `widgets.active.bg_fill` in the applied `Visuals`. Synthetic.
    pub active: Color32,
    /// `true` when the palette is the dark variant. `apply` uses this
    /// to seed `Visuals::dark()` vs `Visuals::light()` so widget
    /// styles that look at `visuals.dark_mode` (egui sliders, text
    /// edits, etc.) pick the right defaults.
    pub dark_mode: bool,
}

/// Dark Fluent palette — identical RGB values to Slint's
/// `FluentPalette` when `color-scheme == Dark`. Alpha-bearing colours
/// (`border`) are stored premultiplied because that's what `Color32`'s
/// const constructors accept; the on-screen result is the same as
/// Slint's unpremultiplied `#RRGGBBAA` values.
pub const DARK: Palette = Palette {
    background: Color32::from_rgb(0x1C, 0x1C, 0x1C),
    foreground: Color32::WHITE,
    alternate_background: Color32::from_rgb(0x2C, 0x2C, 0x2C),
    alternate_foreground: Color32::WHITE,
    // Slint stores this as #FFFFFF0F (white at ~6 % alpha against the
    // dark background); pre-composited against `background` it lands
    // here, which is what egui sees once the widget paints.
    control_background: Color32::from_rgb(0x2E, 0x2E, 0x2E),
    control_foreground: Color32::WHITE,
    accent_background: Color32::from_rgb(0x60, 0xCD, 0xFF),
    accent_foreground: Color32::BLACK,
    selection_background: Color32::from_rgb(0x00, 0x78, 0xD4),
    selection_foreground: Color32::BLACK,
    // Slint #FFFFFF14 ≡ premultiplied (0x14, 0x14, 0x14, 0x14).
    border: Color32::from_rgba_premultiplied(0x14, 0x14, 0x14, 0x14),
    hover: Color32::from_rgb(0x38, 0x38, 0x38),
    active: Color32::from_rgb(0x44, 0x44, 0x44),
    dark_mode: true,
};

/// Light Fluent palette — identical RGB values to Slint's
/// `FluentPalette` when `color-scheme == Light`.
pub const LIGHT: Palette = Palette {
    background: Color32::from_rgb(0xFA, 0xFA, 0xFA),
    // Slint #000000E6 ≡ premultiplied (0, 0, 0, 0xE6).
    foreground: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0xE6),
    alternate_background: Color32::from_rgb(0xF0, 0xF0, 0xF0),
    alternate_foreground: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0xE6),
    // Slint stores this as #FFFFFFB3 — semi-opaque white over the
    // light background. The pre-composited equivalent is essentially
    // pure white, which is what egui's controls render as.
    control_background: Color32::WHITE,
    control_foreground: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0xE6),
    accent_background: Color32::from_rgb(0x00, 0x5F, 0xB8),
    accent_foreground: Color32::WHITE,
    selection_background: Color32::from_rgb(0x00, 0x78, 0xD4),
    selection_foreground: Color32::WHITE,
    // Slint #00000073 ≡ premultiplied (0, 0, 0, 0x73).
    border: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x73),
    hover: Color32::from_rgb(0xE0, 0xE0, 0xE0),
    active: Color32::from_rgb(0xD0, 0xD0, 0xD0),
    dark_mode: false,
};

/// Currently-applied palette. Read by the chip / title / row helpers
/// that can't take the original `Palette` value as a parameter.
/// Defaults to [`DARK`] so widgets render reasonably even when
/// [`apply`] was never called.
static ACTIVE: RwLock<Palette> = RwLock::new(DARK);

/// Snapshot of the palette last passed to [`apply`].
pub fn active() -> Palette {
    ACTIVE.read().expect("theme palette lock").clone()
}

/// Apply `palette` to `ctx` — sets fonts, visuals, and spacing in one
/// call. Run once during app construction, typically inside the
/// `eframe::run_native` builder.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    *ACTIVE.write().expect("theme palette lock") = palette.clone();
    // Drop the bundled emoji fonts. They're the heaviest blobs in the
    // default set (~1.5 MB of raw glyph data the font system would
    // otherwise keep parsed in memory) and Infinitier renders no
    // emoji. Removing by name is a no-op if egui ever renames them,
    // so it can't accidentally blank the Latin text fonts.
    let mut fonts = egui::FontDefinitions::default();
    for emoji in ["NotoEmoji-Regular", "emoji-icon-font"] {
        fonts.font_data.remove(emoji);
        for family in fonts.families.values_mut() {
            family.retain(|name| name != emoji);
        }
    }
    ctx.set_fonts(fonts);

    let mut visuals = if palette.dark_mode {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.background;
    visuals.extreme_bg_color = palette.control_background;

    visuals.widgets.noninteractive.bg_fill = palette.alternate_background;
    visuals.widgets.noninteractive.weak_bg_fill = palette.alternate_background;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.inactive.bg_fill = palette.alternate_background;
    visuals.widgets.inactive.weak_bg_fill = palette.alternate_background;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.hovered.bg_fill = palette.hover;
    visuals.widgets.hovered.weak_bg_fill = palette.hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.active.bg_fill = palette.active;
    visuals.widgets.active.weak_bg_fill = palette.active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.border);

    // Selection: accent_background at low alpha for the fill, full
    // accent for the stroke — same construction the Slint widgets
    // use under the hood.
    let [r, g, b, _] = palette.accent_background.to_array();
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(r, g, b, 60);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent_background);

    visuals.override_text_color = Some(palette.foreground);
    visuals.hyperlink_color = palette.accent_background;

    let mut style = (*ctx.global_style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(8);
    style.text_styles = std::collections::BTreeMap::from([
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
    ]);
    ctx.set_global_style(style);
}

/// Borderless rounded "card" — fill matches `alternate_background` of
/// whichever palette is currently applied. Used by every section-style
/// widget in the keeper. Reads from `ui.style().visuals` so the same
/// helper works under both [`DARK`] and [`LIGHT`].
pub fn card_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = &ui.style().visuals;
    egui::Frame::new()
        .fill(visuals.widgets.noninteractive.bg_fill)
        .inner_margin(egui::Margin::same(12))
        .corner_radius(egui::CornerRadius::same(6))
}

/// Card / section title text — bold, one step larger than body so the
/// section header reads at the same visual weight as the Slint
/// version's `font-size: 14px; font-weight: 700` strapline.
pub fn card_title(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).strong().size(14.5)
}

/// Visual variant for [`chip`] — picks the fill rendered on the
/// unselected state. Selected is always `accent_background` with
/// `accent_foreground` text on top, matching the Slint widgets.
#[derive(Clone, Copy)]
pub enum ChipKind {
    /// Tab-strip style: unselected pills sit on a `control_background`
    /// fill (a faint button-like rectangle).
    Tab,
    /// List-row style: unselected rows are transparent, so the
    /// surrounding panel colour shows through.
    Row,
}

/// Pill-shaped selectable button. Selected state paints the accent
/// rectangle Slint widgets use; unselected matches [`ChipKind`].
///
/// `ChipKind::Row` stretches the chip to the available width so list
/// items align flush with the side panel — matches the way Slint's
/// `ListView` paints a full-width selection rect.
pub fn chip(ui: &mut egui::Ui, text: &str, selected: bool, kind: ChipKind) -> egui::Response {
    let palette = active();
    let (fill, text_color) = if selected {
        (palette.accent_background, palette.accent_foreground)
    } else {
        let unselected_fill = match kind {
            ChipKind::Tab => palette.control_background,
            ChipKind::Row => Color32::TRANSPARENT,
        };
        (unselected_fill, palette.foreground)
    };
    let rich = egui::RichText::new(text).color(text_color);
    let mut button = egui::Button::new(rich)
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(4));
    if matches!(kind, ChipKind::Row) {
        // Full-width row: the inner text stays left-aligned because
        // egui's Button centres atoms; we lean on `Align::Min` via
        // `min_size` + the surrounding layout below.
        button = button.min_size(egui::vec2(ui.available_width(), 24.0));
    }
    match kind {
        ChipKind::Row => ui
            .with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                ui.add(button)
            })
            .inner,
        ChipKind::Tab => ui.add(button),
    }
}

/// One row inside a [`card_frame`]: label left, value right-aligned
/// to the card's inner edge — same layout Slint's `Row` widget paints
/// via `Text { horizontal-stretch: 1 } + Text { font-weight: 700 }`.
///
/// Run inside any vertical layout. Each call paints exactly one row
/// and consumes the available horizontal space.
pub fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    let palette = active();
    // Slint achieves the "muted label" look with `opacity: 0.65`; egui
    // doesn't have a per-widget opacity, so flatten against the card
    // background up-front.
    let muted = mix(palette.foreground, palette.alternate_background, 0.35);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 0.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(egui::RichText::new(label).color(muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(value).strong());
            });
        },
    );
}

/// Blend `a` toward `b` by `t` (0 = pure `a`, 1 = pure `b`).
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let blend = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    let [ar, ag, ab, _] = a.to_array();
    let [br, bg, bb, _] = b.to_array();
    Color32::from_rgb(blend(ar, br), blend(ag, bg), blend(ab, bb))
}
