//! BAM animation viewer. GPUI port of the egui `BamViewer`.
//!
//! Shape:
//! - Picture area (scaled-to-fit, never upscale, centred) showing the
//!   composited current frame.
//! - Control strip: prev/next cycle, prev/next frame, play/pause.
//! - Info bar: frame size, center, file size, BAM variant, frame and
//!   cycle counts, data origin.
//!
//! Texture cache: one `Arc<RenderImage>` per visited `(cycle,
//! frame_in_cycle)` pair, built on demand from
//! `ImportedBam::render_frame_centered` and converted RGBA→BGRA in
//! place (gpui's renderer expects BGRA — see `gpui/elements/img.rs`).
//!
//! Playback: when `playing`, the wall-clock `epoch` + `anchor_frame`
//! drive the current frame index each tick; we call
//! `window.request_animation_frame()` from the render fn so the
//! window keeps repainting at the BAM frame rate.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use bytesize::ByteSize;
use gpui::{
    AnyElement, Context, IntoElement, ObjectFit, ParentElement, RenderImage, Styled,
    StyledImage as _, Window, div, img, px,
};
use gpui_component::{ActiveTheme, Disableable, Sizable, button::Button, h_flex, v_flex};
use image::Frame;
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::bam::ImportedBam,
    resource::bam::{BamV1, Type},
};
use smallvec::SmallVec;

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

pub struct BamViewer {
    bam: ImportedBam,
    selected_cycle: usize,
    selected_frame_in_cycle: usize,
    /// Lazy BGRA texture cache keyed on (cycle, frame_in_cycle). The
    /// composited frame can be expensive to build (palette dispatch +
    /// per-cycle canvas alignment), so we only do it once per visited
    /// pair and re-use the `Arc<RenderImage>` thereafter.
    texture_cache: HashMap<(usize, usize), Arc<RenderImage>>,
    /// `Some` while looping; wall-clock anchor used to derive which
    /// frame is due. Cleared when paused.
    playback: Option<Playback>,
}

struct Playback {
    epoch: Instant,
    anchor_frame: usize,
}

impl BamViewer {
    pub fn new(bam: ImportedBam) -> Self {
        Self {
            bam,
            selected_cycle: 0,
            selected_frame_in_cycle: 0,
            texture_cache: HashMap::new(),
            playback: None,
        }
    }

    fn frames_in_selected_cycle(&self) -> usize {
        self.bam
            .cycles
            .get(self.selected_cycle)
            .map(|c| c.frame_indices.len())
            .unwrap_or(0)
    }

    fn current_global_frame_index(&self) -> Option<usize> {
        let cycle = self.bam.cycles.get(self.selected_cycle)?;
        cycle
            .frame_indices
            .get(self.selected_frame_in_cycle)
            .copied()
    }

    /// Resolve / build the texture for the current selection. The
    /// composited buffer is RGBA8 from the importer; we swap R↔B in
    /// place before handing it to gpui.
    fn current_texture(&mut self) -> Option<Arc<RenderImage>> {
        let key = (self.selected_cycle, self.selected_frame_in_cycle);
        if let Some(tex) = self.texture_cache.get(&key) {
            return Some(tex.clone());
        }
        let composed = self
            .bam
            .render_frame_centered(self.selected_cycle, self.selected_frame_in_cycle)?;
        let mut buffer = composed;
        // RGBA → BGRA (gpui's renderer expects BGRA). Also flatten
        // any *fully* transparent pixel's RGB to zero — BAM v1 stores
        // its "magic green" transparency colour as `(0, 255, 0, 0)`
        // and gpui's renderer samples the texture *before* alpha is
        // applied, so without this the green channel bleeds into
        // sprite edges as a halo when the frame is scaled (egui
        // doesn't show this because it premultiplies on upload).
        for pixel in buffer.chunks_exact_mut(4) {
            if pixel[3] == 0 {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
            } else {
                pixel.swap(0, 2);
            }
        }
        let frame = Frame::new(buffer);
        let tex = Arc::new(RenderImage::new(SmallVec::from_elem(frame, 1)));
        self.texture_cache.insert(key, tex.clone());
        Some(tex)
    }

    /// Re-seat the playback clock to the current selection. Called
    /// every time the user touches the cycle / frame selectors so
    /// playback continues from where they left it.
    fn rebase_playback(&mut self) {
        if let Some(p) = self.playback.as_mut() {
            p.epoch = Instant::now();
            p.anchor_frame = self.selected_frame_in_cycle;
        }
    }

    /// Advance `selected_frame_in_cycle` from wall-clock elapsed.
    fn tick_playback(&mut self) {
        let Some(p) = self.playback.as_ref() else {
            return;
        };
        let len = self.frames_in_selected_cycle();
        if len == 0 {
            return;
        }
        let elapsed = p.epoch.elapsed().as_nanos() as u64;
        let ticks = elapsed / BamV1::DEFAULT_FRAME_DURATION.as_nanos() as u64;
        self.selected_frame_in_cycle = (p.anchor_frame + ticks as usize) % len;
    }
}

impl ResourceViewerTrait for BamViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        // Advance the frame counter from the playback clock before
        // drawing so the rendered frame matches the wall-clock tick.
        self.tick_playback();

        // Copy out the theme colour we paint statically. Hsla is
        // `Copy`, so we can release the immutable `cx` borrow before
        // the `control_strip` / `info_bar` calls below take it
        // mutably (they touch `cx.theme()` + `cx.listener`).
        let border = cx.theme().border;
        let texture = self.current_texture();

        let image_area = picture_area(texture);
        let controls = control_strip(self, cx);
        let info = info_bar(self, resource, cx);

        // Keep the window repainting while playing.
        if self.playback.is_some() {
            window.request_animation_frame();
        }

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(image_area)
            .child(controls)
            .child(div().h_px().bg(border))
            .child(info)
            .into_any_element()
    }
}

/// Image slot using the same absolute-positioned `img` trick the
/// `ImageViewer` does, so taffy can't expand the slot to satisfy the
/// composited frame's intrinsic aspect ratio.
fn picture_area(texture: Option<Arc<RenderImage>>) -> impl IntoElement {
    let mut slot = div()
        .flex_1()
        .min_h_0()
        .w_full()
        .relative()
        .overflow_hidden();
    if let Some(tex) = texture {
        slot = slot.child(
            img(tex)
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .size_full()
                .object_fit(ObjectFit::ScaleDown),
        );
    } else {
        slot = slot.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .flex()
                .items_center()
                .justify_center()
                .child("No frames to display"),
        );
    }
    slot
}

/// Selector strip: cycle prev/next, frame prev/next, play/pause.
/// Each control mutates the viewer through `cx.listener`, then
/// re-bases the playback clock so loops don't jump when the user
/// scrubs by hand.
fn control_strip(viewer: &BamViewer, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let cycle_count = viewer.bam.cycles.len();
    let frames_in_cycle = viewer.frames_in_selected_cycle();
    let cycle_label = if cycle_count == 0 {
        "—".to_string()
    } else {
        format!(
            "Cycle {} / {} ({} frames)",
            viewer.selected_cycle,
            cycle_count.saturating_sub(1),
            frames_in_cycle
        )
    };
    let frame_label = if frames_in_cycle == 0 {
        "Frame — / —".to_string()
    } else {
        format!(
            "Frame {} / {}",
            viewer.selected_frame_in_cycle,
            frames_in_cycle.saturating_sub(1)
        )
    };
    let global_idx_label = match viewer.current_global_frame_index() {
        Some(i) => format!("(frame #{i})"),
        None => String::new(),
    };
    let is_playing = viewer.playback.is_some();
    let can_play = frames_in_cycle > 1;
    let play_label = if is_playing { "Pause" } else { "Play" };

    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(
            Button::new("bam-prev-cycle")
                .label("◀ Cycle")
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = bam_viewer_mut(this);
                    if viewer.bam.cycles.is_empty() {
                        return;
                    }
                    let last = viewer.bam.cycles.len() - 1;
                    viewer.selected_cycle = if viewer.selected_cycle == 0 {
                        last
                    } else {
                        viewer.selected_cycle - 1
                    };
                    let len = viewer.frames_in_selected_cycle();
                    if viewer.selected_frame_in_cycle >= len {
                        viewer.selected_frame_in_cycle = len.saturating_sub(1);
                    }
                    viewer.rebase_playback();
                    cx.notify();
                })),
        )
        .child(div().min_w(px(150.)).child(cycle_label))
        .child(
            Button::new("bam-next-cycle")
                .label("Cycle ▶")
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = bam_viewer_mut(this);
                    if viewer.bam.cycles.is_empty() {
                        return;
                    }
                    let last = viewer.bam.cycles.len() - 1;
                    viewer.selected_cycle = if viewer.selected_cycle >= last {
                        0
                    } else {
                        viewer.selected_cycle + 1
                    };
                    let len = viewer.frames_in_selected_cycle();
                    if viewer.selected_frame_in_cycle >= len {
                        viewer.selected_frame_in_cycle = len.saturating_sub(1);
                    }
                    viewer.rebase_playback();
                    cx.notify();
                })),
        )
        .child(div().w_4())
        .child(
            Button::new("bam-prev-frame")
                .label("◀")
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = bam_viewer_mut(this);
                    let len = viewer.frames_in_selected_cycle();
                    if len == 0 {
                        return;
                    }
                    viewer.selected_frame_in_cycle = if viewer.selected_frame_in_cycle == 0 {
                        len - 1
                    } else {
                        viewer.selected_frame_in_cycle - 1
                    };
                    viewer.rebase_playback();
                    cx.notify();
                })),
        )
        .child(div().min_w(px(110.)).child(frame_label))
        .child(
            Button::new("bam-next-frame")
                .label("▶")
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = bam_viewer_mut(this);
                    let len = viewer.frames_in_selected_cycle();
                    if len == 0 {
                        return;
                    }
                    viewer.selected_frame_in_cycle = (viewer.selected_frame_in_cycle + 1) % len;
                    viewer.rebase_playback();
                    cx.notify();
                })),
        )
        .child(div().w_4())
        .child({
            let mut btn = Button::new("bam-play").label(play_label).small();
            if !can_play {
                btn = btn.disabled(true);
            }
            btn.on_click(cx.listener(|this, _, _, cx| {
                let viewer = bam_viewer_mut(this);
                if viewer.playback.is_some() {
                    viewer.playback = None;
                } else if viewer.frames_in_selected_cycle() > 1 {
                    viewer.playback = Some(Playback {
                        epoch: Instant::now(),
                        anchor_frame: viewer.selected_frame_in_cycle,
                    });
                }
                cx.notify();
            }))
        })
        .child(div().flex_1())
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(global_idx_label),
        )
}

/// Bottom info bar — mirrors the cells the egui viewer paints.
fn info_bar(
    viewer: &BamViewer,
    resource: &GameResource,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();

    let current = viewer
        .current_global_frame_index()
        .and_then(|i| viewer.bam.frames.get(i));
    let (frame_w, frame_h, center_x, center_y) = match current {
        Some(f) => (f.width, f.height, f.center_x, f.center_y),
        None => (0, 0, 0, 0),
    };
    let bam_type_label = match viewer.bam.bam_type {
        Type::BamV1 => "BAM V1",
        Type::BamV2 => "BAM V2",
        Type::BamC => "BAMC",
    };
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
        .child(cell(format!("{frame_w} × {frame_h} px")))
        .child(separator(theme.border))
        .child(cell(format!("center ({center_x}, {center_y})")))
        .child(separator(theme.border))
        .child(cell(file_size))
        .child(separator(theme.border))
        .child(cell(bam_type_label.to_string()))
        .child(separator(theme.border))
        .child(cell(format!("{} frames", viewer.bam.frames.len())))
        .child(separator(theme.border))
        .child(cell(format!("{} cycles", viewer.bam.cycles.len())))
        .child(separator(theme.border))
        .child(cell(origin))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}

/// Pull the currently-cached BAM viewer out of the dispatcher cache.
/// Click handlers run inside `cx.listener` on `ExplorerApp`, not on
/// the viewer itself, so we walk through the `Box<dyn …>` and
/// downcast back to the concrete type.
fn bam_viewer_mut(app: &mut ExplorerApp) -> &mut BamViewer {
    let trait_obj = &mut app
        .viewer
        .inner
        .as_mut()
        .expect("BAM click fired without an active viewer")
        .viewer;
    (trait_obj.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<BamViewer>()
        .expect("active viewer is not a BamViewer")
}
