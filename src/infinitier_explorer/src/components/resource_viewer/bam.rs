use super::ResourceViewerTrait;
use bytesize::ByteSize;
use eframe::egui::{self, TextureHandle};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::bam::{Bam, BamV1},
};
use std::time::Instant;

pub struct BamViewer {
    inner: BamViewerInner,
}

enum BamViewerInner {
    V1(BamV1View),
    V2,
}

struct BamV1View {
    bam: BamV1,
    texture: TextureHandle,
    selected_cycle: usize,
    selected_frame_in_cycle: usize,
    /// (cycle, frame_in_cycle) currently uploaded to `texture`.
    rendered: Option<(usize, usize)>,
    /// `Some` while looping; carries the wall-clock anchor used to
    /// derive which frame is due. Cleared when paused/stopped.
    playback: Option<Playback>,
}

struct Playback {
    /// Wall-clock instant of the anchor.
    epoch: Instant,
    /// `selected_frame_in_cycle` at `epoch`. Future frame indices are
    /// derived as `(anchor_frame + elapsed / BamV1::DEFAULT_FRAME_DURATION) % len`.
    anchor_frame: usize,
}

impl BamViewer {
    pub fn new(bam: Bam, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
        let inner = match bam {
            Bam::V1(bam) => BamViewerInner::V1(BamV1View::new(bam, ui, resource_id)),
            Bam::V2(_) => BamViewerInner::V2,
        };
        Self { inner }
    }
}

impl BamV1View {
    fn new(bam: BamV1, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
        let texture = ui.ctx().load_texture(
            format!("bam_{resource_id}"),
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
            egui::TextureOptions::default(),
        );

        let mut view = Self {
            bam,
            texture,
            selected_cycle: 0,
            selected_frame_in_cycle: 0,
            rendered: None,
            playback: None,
        };
        view.refresh_texture();
        view
    }

    /// Resolve the current (cycle, frame_in_cycle) selection to a global
    /// frame index in `bam.frames`.
    fn current_global_frame_index(&self) -> Option<usize> {
        let cycle = self.bam.cycles.get(self.selected_cycle)?;
        cycle
            .frame_indices
            .get(self.selected_frame_in_cycle)
            .copied()
    }

    /// Re-upload the current frame to the GPU texture if the selection
    /// changed since the last upload. The frame is composited into the
    /// cycle's shared canvas so its anchor stays pinned across frames.
    /// When the current selection has no renderable frame (no cycles, or
    /// the cycle's `frame_indices` is empty) the texture is cleared so
    /// the previous frame doesn't linger on screen.
    fn refresh_texture(&mut self) {
        let key = (self.selected_cycle, self.selected_frame_in_cycle);
        if self.rendered == Some(key) {
            return;
        }
        self.rendered = Some(key);
        match self
            .bam
            .render_frame_centered(self.selected_cycle, self.selected_frame_in_cycle)
        {
            Some(image) => {
                let color = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                );
                self.texture.set(color, egui::TextureOptions::default());
            }
            None => {
                self.texture.set(
                    egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
                    egui::TextureOptions::default(),
                );
            }
        }
    }

    fn frames_in_selected_cycle(&self) -> usize {
        self.bam
            .cycles
            .get(self.selected_cycle)
            .map(|c| c.frame_indices.len())
            .unwrap_or(0)
    }

    /// Anchor the playback clock to the current selection. Called every
    /// time the user touches the cycle/frame selectors so playback
    /// resumes from where they left it instead of jumping.
    fn rebase_playback(&mut self) {
        if let Some(p) = self.playback.as_mut() {
            p.epoch = Instant::now();
            p.anchor_frame = self.selected_frame_in_cycle;
        }
    }

    /// Advance `selected_frame_in_cycle` based on wall-clock elapsed
    /// since the playback anchor. No-op when paused or when the current
    /// cycle is empty.
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
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        match &mut self.inner {
            BamViewerInner::V1(view) => show_v1(view, ui, resource),
            BamViewerInner::V2 => {
                ui.label("BAM V2 viewer not implemented yet");
            }
        }
    }
}

fn show_v1(view: &mut BamV1View, ui: &mut egui::Ui, resource: &GameResource) {
    // Advance the frame counter from the playback clock before drawing.
    view.tick_playback();

    // ── Bottom info bar ───────────────────────────────────────────────────
    let global_idx = view.current_global_frame_index();
    let current_frame = global_idx.and_then(|i| view.bam.frames.get(i));
    let (frame_w, frame_h, center_x, center_y) = match current_frame {
        Some(f) => (f.width, f.height, f.center_x, f.center_y),
        None => (0, 0, 0, 0),
    };
    let frames_count = view.bam.frames.len();
    let cycles_count = view.bam.cycles.len();
    let palette_len = view.bam.palette.len();

    egui::Panel::bottom("bam_info_panel").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("{frame_w} × {frame_h} px"));
            ui.separator();
            ui.label(format!("center ({center_x}, {center_y})"));
            ui.separator();
            match resource.file_size {
                Some(size) => {
                    ui.label(ByteSize(size).to_string());
                }
                None => {
                    ui.label("? B");
                }
            }
            ui.separator();
            ui.label("BAM V1");
            ui.separator();
            ui.label(format!("{frames_count} frames"));
            ui.separator();
            ui.label(format!("{cycles_count} cycles"));
            ui.separator();
            ui.label(format!("{palette_len} colors"));
            ui.separator();
            match &resource.data_origin {
                DataOrigin::Bif { name } => {
                    ui.label(format!("BIF: {name}"));
                }
                DataOrigin::Dir { name, path } => {
                    ui.label(format!("{name}: {}", path.path().display()));
                }
                DataOrigin::Missing => {
                    ui.label("Missing");
                }
            }
        });
    });

    // ── Selector bar (above the info bar) ─────────────────────────────────
    let mut toggle_play = false;
    egui::Panel::bottom("bam_selector_panel").show_inside(ui, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let is_playing = view.playback.is_some();
            let can_play = view.frames_in_selected_cycle() > 1;
            let label = if is_playing {
                "⏸  Pause"
            } else {
                "▶  Play"
            };
            if ui.add_enabled(can_play, egui::Button::new(label)).clicked() {
                toggle_play = true;
            }

            ui.separator();

            ui.label("Cycle:");
            let cycles_count = view.bam.cycles.len();
            let cycle_label = if cycles_count == 0 {
                "—".to_string()
            } else {
                let frames_in_cycle = view
                    .bam
                    .cycles
                    .get(view.selected_cycle)
                    .map(|c| c.frame_indices.len())
                    .unwrap_or(0);
                format!(
                    "{} / {} ({} frames)",
                    view.selected_cycle, cycles_count, frames_in_cycle
                )
            };
            let mut cycle_changed = false;
            egui::ComboBox::from_id_salt("bam_cycle_combo")
                .selected_text(cycle_label)
                .show_ui(ui, |ui| {
                    for (i, cycle) in view.bam.cycles.iter().enumerate() {
                        let label = format!("{} ({} frames)", i, cycle.frame_indices.len());
                        if ui
                            .selectable_label(view.selected_cycle == i, label)
                            .clicked()
                        {
                            view.selected_cycle = i;
                            let len = cycle.frame_indices.len();
                            if len == 0 {
                                view.selected_frame_in_cycle = 0;
                            } else if view.selected_frame_in_cycle >= len {
                                view.selected_frame_in_cycle = len - 1;
                            }
                            cycle_changed = true;
                        }
                    }
                });
            if cycle_changed {
                view.rebase_playback();
            }

            ui.separator();

            ui.label("Frame:");
            let frames_in_cycle = view.frames_in_selected_cycle();
            if frames_in_cycle > 1 {
                let max = frames_in_cycle - 1;
                let response = ui.add(
                    egui::Slider::new(&mut view.selected_frame_in_cycle, 0..=max)
                        .text(format!("/ {max}")),
                );
                if response.changed() {
                    view.rebase_playback();
                }
            } else if frames_in_cycle == 1 {
                view.selected_frame_in_cycle = 0;
                ui.label("0 / 0");
            } else {
                ui.label("—");
            }

            if let Some(idx) = view.current_global_frame_index() {
                ui.separator();
                ui.label(format!("(frame #{idx})"));
            }
        });
        ui.add_space(4.0);
    });

    if toggle_play {
        if view.playback.is_some() {
            view.playback = None;
        } else if view.frames_in_selected_cycle() > 1 {
            view.playback = Some(Playback {
                epoch: Instant::now(),
                anchor_frame: view.selected_frame_in_cycle,
            });
        }
    }

    // Re-render if the selection changed this tick.
    view.refresh_texture();

    // Keep repainting while playing so frames advance on schedule.
    if view.playback.is_some() {
        ui.ctx()
            .request_repaint_after(BamV1::DEFAULT_FRAME_DURATION);
    }

    // ── Central area: the rendered frame ──────────────────────────────────
    if view.current_global_frame_index().is_none() {
        ui.centered_and_justified(|ui| {
            ui.label("No frames to display");
        });
        return;
    }

    let available = ui.available_size();
    let natural = view.texture.size_vec2();
    if natural.x <= 0.0 || natural.y <= 0.0 {
        return;
    }
    let scale = (available.x / natural.x)
        .min(available.y / natural.y)
        .min(1.0);
    let display = natural * scale;

    let y_offset = ((available.y - display.y) / 2.0).max(0.0);
    if y_offset > 0.0 {
        ui.add_space(y_offset);
    }
    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.add(egui::Image::new(&view.texture).fit_to_exact_size(display));
    });
}
