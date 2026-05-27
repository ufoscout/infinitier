//! TTF viewer. GPUI port of the egui `TtfViewer`.
//!
//! Mirrors the egui viewer: header with the typeface's display name
//! drawn in its own font at 36 px, a metadata grid (Version, Designer,
//! Foundry, Copyright, PostScript), and a sample-text playground that
//! renders a fixed sentence at five sizes so the reader can preview
//! the font across typical UI ranges.
//!
//! Font installation goes through `cx.text_system().add_fonts(...)` —
//! gpui's runtime-font-registration API — and uses the TTF's own
//! `family_name` so `font_family(name)` resolves on the very next
//! repaint. We install lazily on the first render so the dispatcher
//! cache doesn't need to thread `cx` through `build_viewer`.
//!
//! The egui version makes the sample text editable via `TextEdit`.
//! gpui-component's `Input` widget requires its own `Entity` and a
//! focus dance — overkill for a one-line preview field, so this port
//! ships with a fixed sentence. The sample text constant lives here
//! so a future patch can swap it for an `Input` without touching the
//! layout.

use std::borrow::Cow;

use bytesize::ByteSize;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::ttf::Ttf,
};

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

const SAMPLE_TEXT: &str = "The quick brown fox jumps over the lazy dog. 0123456789";
const SAMPLE_SIZES_PX: &[f32] = &[12.0, 16.0, 24.0, 36.0, 48.0];

pub struct TtfViewer {
    ttf: Ttf,
    /// Cached as `SharedString` because we hand it to `font_family()`
    /// on every frame and each render call would otherwise allocate.
    family: SharedString,
    /// `false` until the first `render` call has installed the font
    /// in gpui's text system; flipped to `true` so subsequent renders
    /// skip the registration. `add_fonts` is idempotent on the same
    /// bytes anyway, but skipping saves the alloc.
    installed: bool,
}

impl TtfViewer {
    pub fn new(ttf: Ttf) -> Self {
        let family = SharedString::from(ttf.family_name.clone());
        Self {
            ttf,
            family,
            installed: false,
        }
    }

    fn install_if_needed(&mut self, cx: &mut Context<ExplorerApp>) {
        if self.installed {
            return;
        }
        // Clone the raw bytes for gpui — the `Arc<Vec<u8>>` payload
        // stays shared with anything else holding the `Ttf` (the
        // importer's cache, the info-bar metadata, …).
        let bytes: Vec<u8> = (*self.ttf.raw).clone();
        match cx.text_system().add_fonts(vec![Cow::Owned(bytes)]) {
            Ok(()) => self.installed = true,
            Err(e) => log::warn!("[ttf] failed to install font {}: {e}", self.family),
        }
    }
}

impl ResourceViewerTrait for TtfViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        _window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        self.install_if_needed(cx);

        let border = cx.theme().border;

        let scroll = div()
            .id("ttf-scroll")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .p_4()
            .child(header(self, cx))
            .child(div().h_4())
            .child(div().h_px().bg(border))
            .child(div().h_4())
            .child(metadata_grid(&self.ttf, cx))
            .child(div().h_4())
            .child(div().h_px().bg(border))
            .child(div().h_4())
            .child(sample_text(self, cx));

        let info = info_bar(&self.ttf, resource, cx);

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(scroll)
            .child(div().h_px().bg(border))
            .child(info)
            .into_any_element()
    }
}

/// Header — typeface's full name rendered in its own font at 36 px.
fn header(viewer: &TtfViewer, _cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    div()
        .font_family(viewer.family.clone())
        .text_size(px(36.))
        .child(viewer.ttf.full_name.clone())
}

/// Two-column metadata grid (label / value) for the optional `name`-table
/// strings. Same five rows the egui viewer surfaces, when present.
fn metadata_grid(ttf: &Ttf, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let mut details: Vec<(&'static str, String)> = Vec::new();
    if let Some(v) = &ttf.version {
        details.push(("Version", v.clone()));
    }
    if let Some(d) = &ttf.designer {
        details.push(("Designer", d.clone()));
    }
    if let Some(m) = &ttf.manufacturer {
        details.push(("Foundry", m.clone()));
    }
    if let Some(c) = &ttf.copyright {
        details.push(("Copyright", c.clone()));
    }
    if let Some(ps) = &ttf.postscript_name {
        details.push(("PostScript", ps.clone()));
    }

    let mut col = v_flex().w_full().gap_1();
    for (label, value) in details {
        col = col.child(
            h_flex()
                .w_full()
                .gap_3()
                .child(
                    div()
                        .min_w(px(110.))
                        .font_weight(FontWeight::BOLD)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(theme.muted_foreground)
                        .child(value),
                ),
        );
    }
    col
}

/// Sample-text playground — fixed sentence rendered in the font at
/// five sizes, each row prefixed with a muted size label.
fn sample_text(viewer: &TtfViewer, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let mut col = v_flex().w_full().gap_2().child(
        div()
            .font_weight(FontWeight::BOLD)
            .text_size(px(16.))
            .child("Sample text"),
    );

    for &size in SAMPLE_SIZES_PX {
        col = col.child(
            v_flex()
                .gap_0p5()
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .text_size(px(11.))
                        .child(format!("{size:.0} px")),
                )
                .child(
                    div()
                        .font_family(viewer.family.clone())
                        .text_size(px(size))
                        .child(SharedString::from(SAMPLE_TEXT)),
                ),
        );
    }

    col
}

fn info_bar(
    ttf: &Ttf,
    resource: &GameResource,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();

    let file_size = match resource.file_size {
        Some(size) => ByteSize(size).to_string(),
        None => "? B".to_string(),
    };
    let origin = match &resource.data_origin {
        DataOrigin::Bif { name } => format!("BIF: {name}"),
        DataOrigin::Dir { name, path } => format!("{name}: {}", path.path().display()),
        DataOrigin::Missing => "Missing".to_string(),
    };

    let mut row = h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(cell("TTF".to_string()))
        .child(separator(theme.border))
        .child(cell(format!("{} {}", ttf.family_name, ttf.subfamily_name)))
        .child(separator(theme.border))
        .child(cell(format!("{} glyphs", ttf.glyph_count)))
        .child(separator(theme.border))
        .child(cell(format!(
            "em={} asc={} desc={} line_gap={}",
            ttf.units_per_em, ttf.ascender, ttf.descender, ttf.line_gap,
        )))
        .child(separator(theme.border));
    if ttf.is_monospaced {
        row = row
            .child(cell("monospaced".to_string()))
            .child(separator(theme.border));
    }
    row.child(cell(file_size))
        .child(separator(theme.border))
        .child(cell(origin))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}
