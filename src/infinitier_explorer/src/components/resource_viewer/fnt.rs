//! FNT viewer. GPUI port of the egui `FntViewer`.
//!
//! Mirrors NearInfinity's `FntResource` panel cell-for-cell:
//! - Scrollable centre area containing
//!   - a four-column struct table (Attribute / Value / Offset / Size),
//!   - a one-line note clarifying that FNT is a 4-byte envelope and
//!     the rest of the file is engine-internal opaque data,
//!   - a hex dump of the first 256 bytes past the header.
//! - Bottom info bar with the FNT label, extra-letter count, file
//!   size, body size, and data origin.

use bytesize::ByteSize;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::fnt::{Fnt, HEADER_LEN},
};

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

pub struct FntViewer {
    fnt: Fnt,
}

impl FntViewer {
    pub fn new(fnt: Fnt) -> Self {
        Self { fnt }
    }
}

impl ResourceViewerTrait for FntViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        _window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        let border = cx.theme().border;

        let scroll = div()
            .id("fnt-scroll")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .p_4()
            .child(struct_table(&self.fnt, cx))
            .child(div().h_4())
            .child(format_note(&self.fnt, cx))
            .child(div().h_4())
            .child(body_preview(&self.fnt, cx));

        let info = info_bar(&self.fnt, resource, cx);

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

/// NI-style struct table — Attribute / Value / Offset / Size columns.
/// Same three data rows the egui viewer paints.
fn struct_table(fnt: &Fnt, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let header_bg = theme.secondary;
    let row_bg_alt = theme.muted;

    v_flex()
        .w_full()
        .gap_0()
        .border_1()
        .border_color(theme.border)
        .rounded(theme.radius)
        .child(table_header_row(header_bg))
        .child(table_row(
            "# extra letters",
            fnt.extra_letters_count.to_string(),
            "0 h",
            HEADER_LEN.to_string(),
            row_bg_alt,
        ))
        .child(table_row(
            "Letters",
            fnt.letters_bam.clone(),
            "0 h",
            "8".to_string(),
            theme.transparent,
        ))
        .child(table_row(
            "Extra letters",
            fnt.extra_letters_bmp.clone(),
            "0 h",
            "8".to_string(),
            row_bg_alt,
        ))
}

fn table_header_row(bg: gpui::Hsla) -> impl IntoElement {
    h_flex()
        .w_full()
        .px_3()
        .py_2()
        .gap_3()
        .bg(bg)
        .font_weight(FontWeight::BOLD)
        .child(div().min_w(px(160.)).child("Attribute"))
        .child(div().flex_1().child("Value"))
        .child(div().min_w(px(80.)).child("Offset"))
        .child(div().min_w(px(60.)).child("Size"))
}

fn table_row(
    attribute: impl Into<SharedString>,
    value: impl Into<SharedString>,
    offset: impl Into<SharedString>,
    size: impl Into<SharedString>,
    bg: gpui::Hsla,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .px_3()
        .py_2()
        .gap_3()
        .bg(bg)
        .child(div().min_w(px(160.)).child(attribute.into()))
        .child(div().flex_1().child(value.into()))
        .child(div().min_w(px(80.)).child(offset.into()))
        .child(div().min_w(px(60.)).child(size.into()))
}

/// One-line clarifier — keeps users from wondering why the file is
/// 100 KB on disk but the table only shows three fields.
fn format_note(fnt: &Fnt, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let body_len = fnt.body().len();
    let theme = cx.theme();
    if body_len == 0 {
        return div().child("");
    }
    div().text_color(theme.muted_foreground).child(format!(
        "Note: FNT is a stub. Glyph data lives in {} and {}; the {body_len} bytes \
         past offset 0x04 in this file are engine-internal and not parsed \
         (NearInfinity treats them the same way).",
        fnt.letters_bam, fnt.extra_letters_bmp,
    ))
}

/// First N bytes of the un-parsed body as a hex dump — same content
/// NI's "Raw" tab would surface.
fn body_preview(fnt: &Fnt, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    const PREVIEW_BYTES: usize = 256;
    let theme = cx.theme();

    let body = fnt.body();
    let mut col = v_flex().w_full().gap_1().child(
        div()
            .font_weight(FontWeight::BOLD)
            .text_size(px(16.))
            .child("Raw (post-header body)"),
    );
    if body.is_empty() {
        return col.child("(no body bytes)");
    }
    let shown = body.len().min(PREVIEW_BYTES);
    col = col.child(div().text_color(theme.muted_foreground).child(format!(
        "Showing first {shown} of {} bytes (offset 0x{HEADER_LEN:X} in file).",
        body.len(),
    )));

    let mut dump = String::with_capacity(shown * 4);
    for (i, chunk) in body[..shown].chunks(16).enumerate() {
        dump.push_str(&format!("{:08x}  ", HEADER_LEN + i * 16));
        for b in chunk {
            dump.push_str(&format!("{:02x} ", b));
        }
        for _ in chunk.len()..16 {
            dump.push_str("   ");
        }
        dump.push(' ');
        for &b in chunk {
            dump.push(if (32..127).contains(&b) {
                b as char
            } else {
                '.'
            });
        }
        dump.push('\n');
    }

    col.child(
        div()
            .w_full()
            .p_3()
            .bg(theme.secondary)
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(cx.theme().mono_font_size)
            .child(dump),
    )
}

fn info_bar(
    fnt: &Fnt,
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

    h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(cell("FNT".to_string()))
        .child(separator(theme.border))
        .child(cell(format!(
            "# extra letters: {}",
            fnt.extra_letters_count
        )))
        .child(separator(theme.border))
        .child(cell(file_size))
        .child(separator(theme.border))
        .child(cell(format!("body: {} B (opaque)", fnt.body().len())))
        .child(separator(theme.border))
        .child(cell(origin))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}
