use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::acm::{AcmDecoder, AcmInfo},
};
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

pub struct AcmViewer {
    info: AcmInfo,
    /// Decoded interleaved PCM in `f32` (rodio's native sample type), shared
    /// behind an `Arc` so each Play press can hand the queue a fresh
    /// position-zero source without re-cloning the entire buffer.
    samples: Arc<Vec<f32>>,
    duration: Duration,
    /// Audio output, kept on the viewer so playback persists across UI
    /// frames. `None` if the system has no usable audio device or if
    /// decoding failed — the player UI degrades gracefully in that case.
    audio: Option<Audio>,
    /// Set when decoding the ACM payload failed. Surfaced in the player
    /// area in lieu of the controls.
    decode_error: Option<String>,
}

struct Audio {
    /// Output device handle — must outlive `sink`.
    _stream: MixerDeviceSink,
    sink: Player,
}

impl AcmViewer {
    pub fn new(mut acm: AcmDecoder) -> Self {
        let info = acm.info.clone();
        let (samples, decode_error) = match acm.decode_all() {
            Ok(pcm) => {
                let f32s: Vec<f32> = pcm.into_iter().map(i16_to_f32).collect();
                (Arc::new(f32s), None)
            }
            Err(e) => (Arc::new(Vec::new()), Some(format!("decode failed: {e}"))),
        };

        let duration = compute_duration(&info);
        let audio = init_audio();

        Self {
            info,
            samples,
            duration,
            audio,
            decode_error,
        }
    }
}

#[inline]
fn i16_to_f32(s: i16) -> f32 {
    s as f32 / 32768.0
}

fn compute_duration(info: &AcmInfo) -> Duration {
    if info.rate == 0 || info.channels == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(info.samples() as f64 / info.rate as f64)
}

fn init_audio() -> Option<Audio> {
    let mut device = DeviceSinkBuilder::open_default_sink().ok()?;
    // Avoid a noisy "device dropped while still streaming" log when the
    // viewer is replaced — explorer flips between viewers regularly.
    device.log_on_drop(false);
    let sink = Player::connect_new(device.mixer());
    sink.pause(); // start paused; user presses Play to begin
    Some(Audio {
        _stream: device,
        sink,
    })
}

/// Source that streams pre-decoded `f32` PCM out of a shared buffer.
/// Owning an `Arc<Vec<f32>>` (rather than a `Vec<f32>` per Play) lets us
/// re-create the source after Stop without re-allocating the audio data.
struct PcmSource {
    samples: Arc<Vec<f32>>,
    pos: usize,
    channels: ChannelCount,
    sample_rate: SampleRate,
    total_duration: Duration,
}

impl Iterator for PcmSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = *self.samples.get(self.pos)?;
        self.pos += 1;
        Some(s)
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.pos))
    }
    fn channels(&self) -> ChannelCount {
        self.channels
    }
    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(self.total_duration)
    }
}

fn make_source(
    samples: &Arc<Vec<f32>>,
    info: &AcmInfo,
    duration: Duration,
) -> Option<PcmSource> {
    let channels = NonZero::new(info.channels as u16)?;
    let sample_rate = NonZero::new(info.rate)?;
    if samples.is_empty() {
        return None;
    }
    Some(PcmSource {
        samples: Arc::clone(samples),
        pos: 0,
        channels,
        sample_rate,
        total_duration: duration,
    })
}

impl ResourceViewerTrait for AcmViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        // ── Bottom info bar ────────────────────────────────────────────────
        egui::Panel::bottom("acm_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} Hz", self.info.rate));
                ui.separator();
                ui.label(channels_label(self.info.channels, self.info.acm_channels));
                ui.separator();
                ui.label(format!("{} samples", self.info.samples()));
                ui.separator();
                ui.label(format_duration(self.duration));
                ui.separator();
                ui.label(format!("ACM v{}", self.info.acm_version));
                ui.separator();
                ui.label(format!(
                    "{}×{} block",
                    self.info.acm_rows, self.info.acm_cols
                ));
            });
        });

        // ── Central player ─────────────────────────────────────────────────
        // Bind disjoint fields up-front so the inner closure only borrows
        // what it needs (avoiding the closure-captures-all-of-`self`
        // collision with the `&mut self.audio` borrow below).
        let info = &self.info;
        let samples = &self.samples;
        let duration = self.duration;
        let decode_error = self.decode_error.as_deref();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(24.0);
                    ui.heading("ACM Player");
                    ui.add_space(16.0);

                    if let Some(err) = decode_error {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                        return;
                    }

                    let Some(audio) = self.audio.as_mut() else {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "No audio output device available — playback disabled.",
                        );
                        return;
                    };

                    // ── Progress bar ──
                    let pos = current_position(audio);
                    let total_secs = duration.as_secs_f64();
                    let progress = if total_secs > 0.0 {
                        (pos.as_secs_f64() / total_secs).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    };

                    ui.add(
                        egui::ProgressBar::new(progress)
                            .desired_width(420.0)
                            .text(format!(
                                "{} / {}",
                                format_duration(pos),
                                format_duration(duration)
                            )),
                    );
                    ui.add_space(14.0);

                    // ── Transport buttons ──
                    let is_playing = !audio.sink.is_paused() && !audio.sink.empty();
                    let has_audio = !samples.is_empty();

                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(has_audio, egui::Button::new("▶  Play"))
                            .clicked()
                        {
                            // The queue empties either at first show or after
                            // Stop; in both cases re-attach a fresh source so
                            // playback always resumes (or restarts) from the
                            // beginning of the clip.
                            if audio.sink.empty()
                                && let Some(src) = make_source(samples, info, duration)
                            {
                                audio.sink.append(src);
                            }
                            audio.sink.play();
                        }
                        if ui
                            .add_enabled(is_playing, egui::Button::new("⏸  Pause"))
                            .clicked()
                        {
                            audio.sink.pause();
                        }
                        if ui
                            .add_enabled(has_audio, egui::Button::new("⏹  Stop"))
                            .clicked()
                        {
                            // Drop the queued source — the next Play will
                            // append a fresh one starting at sample 0,
                            // matching the AcmDecoder::reset semantics.
                            audio.sink.stop();
                        }
                    });

                    // While playing, ask egui to repaint so the progress
                    // bar tracks playback in real time without forcing
                    // continuous repaints when paused/stopped.
                    if is_playing {
                        ui.ctx()
                            .request_repaint_after(Duration::from_millis(100));
                    }
                });
            });
    }
}

fn current_position(audio: &Audio) -> Duration {
    if audio.sink.empty() {
        Duration::ZERO
    } else {
        audio.sink.get_pos()
    }
}

fn channels_label(channels: u32, native: u32) -> String {
    if channels == native {
        format!("{channels} ch")
    } else {
        // `OutputChannels` override applied — surface the original count.
        format!("{channels} ch (native {native})")
    }
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs_f64();
    let mins = (total / 60.0).floor() as u64;
    let secs = total - (mins as f64) * 60.0;
    format!("{mins}:{:05.2}", secs)
}
