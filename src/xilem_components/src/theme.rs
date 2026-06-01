//! Design tokens. Mirrors the *idea* of `egui_components`' `Theme`
//! (a small palette + spacing/radius scalars) so callers thread one
//! value through the component constructors. The actual colour values
//! are placeholders for now — the look is customised in a later step;
//! what matters here is that every component reads its colours/metrics
//! from this struct, so re-theming is a one-place change.

use xilem::Color;

/// Palette + metrics shared by every component. `Copy` so it can be
/// passed by value into component constructors without ceremony.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub muted_foreground: Color,
    /// Card / secondary surface background.
    pub surface: Color,
    pub border: Color,
    pub primary: Color,
    pub primary_foreground: Color,
    /// Accent used for the active tab / selected row.
    pub accent: Color,

    /// Corner radius (logical px) for cards, buttons, inputs.
    pub radius: f32,
    /// Default gap between stacked children (logical px).
    pub gap: f32,
    /// Default inner padding (logical px).
    pub padding: f32,
    /// Body font size (logical px).
    pub font_size: f32,
    /// Title / heading font size (logical px).
    pub title_size: f32,
}

impl Theme {
    /// A neutral light placeholder palette. Intentionally plain — the
    /// real shadcn-style theme is a follow-up step.
    pub fn light() -> Self {
        Self {
            background: Color::from_rgb8(0xff, 0xff, 0xff),
            foreground: Color::from_rgb8(0x1a, 0x1a, 0x1a),
            muted_foreground: Color::from_rgb8(0x6b, 0x72, 0x80),
            surface: Color::from_rgb8(0xf4, 0xf4, 0xf5),
            border: Color::from_rgb8(0xd4, 0xd4, 0xd8),
            primary: Color::from_rgb8(0x18, 0x18, 0x1b),
            primary_foreground: Color::from_rgb8(0xfa, 0xfa, 0xfa),
            accent: Color::from_rgb8(0x3b, 0x82, 0xf6),
            radius: 6.0,
            gap: 6.0,
            padding: 10.0,
            font_size: 14.0,
            title_size: 16.0,
        }
    }

    /// A neutral dark placeholder palette.
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb8(0x0a, 0x0a, 0x0a),
            foreground: Color::from_rgb8(0xf2, 0xf2, 0xf2),
            muted_foreground: Color::from_rgb8(0x9a, 0xa0, 0xaa),
            surface: Color::from_rgb8(0x1a, 0x1a, 0x1d),
            border: Color::from_rgb8(0x2e, 0x2e, 0x33),
            primary: Color::from_rgb8(0xfa, 0xfa, 0xfa),
            primary_foreground: Color::from_rgb8(0x18, 0x18, 0x1b),
            accent: Color::from_rgb8(0x3b, 0x82, 0xf6),
            radius: 6.0,
            gap: 6.0,
            padding: 10.0,
            font_size: 14.0,
            title_size: 16.0,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}
