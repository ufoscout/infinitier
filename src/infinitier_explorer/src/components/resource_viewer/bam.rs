use super::ResourceViewerTrait;
use bytesize::ByteSize;
use eframe::egui::{self, TextureHandle};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::bam::ImportedBam,
    resource::bam::{BamV1, Type},
};
use std::time::Instant;

pub struct BamViewer {
    bam: ImportedBam,
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
    pub fn new(bam: ImportedBam, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
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
        // Advance the frame counter from the playback clock before drawing.
        self.tick_playback();

        // ── Bottom info bar ───────────────────────────────────────────────────
        let global_idx = self.current_global_frame_index();
        let current_frame = global_idx.and_then(|i| self.bam.frames.get(i));
        let (frame_w, frame_h, center_x, center_y) = match current_frame {
            Some(f) => (f.width, f.height, f.center_x, f.center_y),
            None => (0, 0, 0, 0),
        };
        let frames_count = self.bam.frames.len();
        let cycles_count = self.bam.cycles.len();
        let bam_type_label = match self.bam.bam_type {
            Type::BamV1 => "BAM V1",
            Type::BamV2 => "BAM V2",
            Type::BamC => "BAMC",
        };

        egui::Panel::bottom("bam_info_panel").show(ui, |ui| {
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
                ui.label(bam_type_label);
                ui.separator();
                ui.label(format!("{frames_count} frames"));
                ui.separator();
                ui.label(format!("{cycles_count} cycles"));
                ui.separator();
                match &resource.data_origin {
                    DataOrigin::Bif { name } => {
                        ui.label(format!("BIF: {name}"));
                    }
                    DataOrigin::Dir { name, path } => {
                        ui.label(format!("{name}: {}", path.path().display()));
                    }
                    DataOrigin::Unhardcoded { folder } => {
                        ui.label(format!("unhardcoded/{folder}"));
                    }
                    DataOrigin::Missing => {
                        ui.label("Missing");
                    }
                }
            });
        });

        // ── Selector bar (above the info bar) ─────────────────────────────────
        let mut toggle_play = false;
        egui::Panel::bottom("bam_selector_panel").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Force every control in this toolbar to one uniform
                // height. A plain horizontal row with mixed-height widgets
                // (the cycle combo and the slider's value box render a few
                // px taller than a plain button) makes egui's centred
                // layout place each successive widget slightly lower — a
                // visible vertical "staircase". Pinning `interact_size.y`
                // to a height that clears the tallest control snaps the
                // button, combo, slider and value box to the same height,
                // so they all centre on one line. The value is the egui
                // button-height floor (text + vertical padding) plus a 2px
                // margin to clear the combo/slider; it's robust across
                // pixels_per_point (1.0–2.0) since it's expressed in points.
                ui.spacing_mut().interact_size.y = ui.spacing().interact_size.y.max(
                    ui.text_style_height(&egui::TextStyle::Button)
                        + 2.0 * ui.spacing().button_padding.y,
                ) + 2.0;
                let is_playing = self.playback.is_some();
                let can_play = self.frames_in_selected_cycle() > 1;
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
                let cycles_count = self.bam.cycles.len();
                let cycle_label = if cycles_count == 0 {
                    "—".to_string()
                } else {
                    let frames_in_cycle = self
                        .bam
                        .cycles
                        .get(self.selected_cycle)
                        .map(|c| c.frame_indices.len())
                        .unwrap_or(0);
                    format!(
                        "{} / {} ({} frames)",
                        self.selected_cycle, cycles_count, frames_in_cycle
                    )
                };
                let mut cycle_changed = false;
                egui::ComboBox::from_id_salt("bam_cycle_combo")
                    .selected_text(cycle_label)
                    .show_ui(ui, |ui| {
                        for (i, cycle) in self.bam.cycles.iter().enumerate() {
                            let label = format!("{} ({} frames)", i, cycle.frame_indices.len());
                            if ui
                                .selectable_label(self.selected_cycle == i, label)
                                .clicked()
                            {
                                self.selected_cycle = i;
                                let len = cycle.frame_indices.len();
                                if len == 0 {
                                    self.selected_frame_in_cycle = 0;
                                } else if self.selected_frame_in_cycle >= len {
                                    self.selected_frame_in_cycle = len - 1;
                                }
                                cycle_changed = true;
                            }
                        }
                    });
                if cycle_changed {
                    self.rebase_playback();
                }

                ui.separator();

                ui.label("Frame:");
                let frames_in_cycle = self.frames_in_selected_cycle();
                if frames_in_cycle > 1 {
                    let max = frames_in_cycle - 1;
                    let response = ui.add(
                        egui::Slider::new(&mut self.selected_frame_in_cycle, 0..=max)
                            .text(format!("/ {max}")),
                    );
                    if response.changed() {
                        self.rebase_playback();
                    }
                } else if frames_in_cycle == 1 {
                    self.selected_frame_in_cycle = 0;
                    ui.label("0 / 0");
                } else {
                    ui.label("—");
                }

                if let Some(idx) = self.current_global_frame_index() {
                    ui.separator();
                    ui.label(format!("(frame #{idx})"));
                }
            });
            ui.add_space(4.0);
        });

        if toggle_play {
            if self.playback.is_some() {
                self.playback = None;
            } else if self.frames_in_selected_cycle() > 1 {
                self.playback = Some(Playback {
                    epoch: Instant::now(),
                    anchor_frame: self.selected_frame_in_cycle,
                });
            }
        }

        // Re-render if the selection changed this tick.
        self.refresh_texture();

        // Keep repainting while playing so frames advance on schedule.
        if self.playback.is_some() {
            ui.ctx()
                .request_repaint_after(BamV1::DEFAULT_FRAME_DURATION);
        }

        // ── Central area: the rendered frame ──────────────────────────────────
        if self.current_global_frame_index().is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("No frames to display");
            });
            return;
        }

        let available = ui.available_size();
        let natural = self.texture.size_vec2();
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
            ui.add(egui::Image::new(&self.texture).fit_to_exact_size(display));
        });
    }
}
