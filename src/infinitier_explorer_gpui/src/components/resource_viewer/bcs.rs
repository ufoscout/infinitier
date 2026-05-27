//! BCS script viewer. GPUI port of the egui `BcsViewer`.
//!
//! Shape: scrollable monospaced BAF source area on top of an info bar
//! that surfaces CR-block count, BAF line count, file size, and data
//! origin — mirrors the egui viewer cell-for-cell. Token colouring
//! goes through `StyledText::with_highlights` driven by the tokenizer
//! in [`super::baf_highlight`]; we pick the dark / light palette from
//! the current `gpui_component::Theme::mode`.

use bytesize::ByteSize;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, StyledText, Styled, Window, div,
};
use gpui_component::{ActiveTheme, ThemeMode, h_flex, v_flex};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::bcs::ImportedBcs,
};

use super::ResourceViewerTrait;
use super::baf_highlight::{self, BafPalette};
use crate::app::ExplorerApp;

pub struct BcsViewer {
    bcs: ImportedBcs,
    /// Pre-shared BAF text. Cached as `SharedString` so the
    /// `StyledText` we hand to gpui every frame can clone the handle
    /// instead of copying the string (BAF scripts can be hundreds of
    /// KB on the larger areas).
    baf_text: SharedString,
}

impl BcsViewer {
    pub fn new(bcs: ImportedBcs) -> Self {
        let baf_text = SharedString::from(bcs.baf.clone());
        Self { bcs, baf_text }
    }
}

impl ResourceViewerTrait for BcsViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        _window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        let palette = match cx.theme().mode {
            ThemeMode::Dark => BafPalette::dark(),
            ThemeMode::Light => BafPalette::light(),
        };
        let mono_family = cx.theme().mono_font_family.clone();
        let mono_size = cx.theme().mono_font_size;
        let border = cx.theme().border;
        let text_color = cx.theme().foreground;

        let highlights = baf_highlight::highlight_ranges(&self.baf_text, &palette);

        let source = div()
            .id("bcs-source-scroll")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .p_3()
            .font_family(mono_family)
            .text_size(mono_size)
            .text_color(text_color)
            .child(StyledText::new(self.baf_text.clone()).with_highlights(highlights));

        let info = info_bar(self, resource, cx);

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(source)
            .child(div().h_px().bg(border))
            .child(info)
            .into_any_element()
    }
}

fn info_bar(
    viewer: &BcsViewer,
    resource: &GameResource,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();

    let cr_count = viewer.bcs.bcs.condition_responses.len();
    let baf_lines = viewer.baf_text.lines().count();
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
        .child(cell(format!("{cr_count} CR blocks")))
        .child(separator(theme.border))
        .child(cell(format!("{baf_lines} BAF lines")))
        .child(separator(theme.border))
        .child(cell(file_size))
        .child(separator(theme.border))
        .child(cell(origin))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}
