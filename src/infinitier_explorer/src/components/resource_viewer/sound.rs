use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    imported_resource::sound::{SoundDecoder, SoundFormat, SoundInfo},
};
use log::error;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use std::collections::VecDeque;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

/// How many `i16` samples the decoder thread reads in one `read_samples`
/// call. A small chunk keeps lock-hold time short and lets the producer
/// react to a Stop request promptly.
const CHUNK_SAMPLES: usize = 2048;

/// Player for any [`SoundDecoder`] — i.e. ACM, RIFF/WAVE, or WAVC.
///
/// Decoding runs on a dedicated background thread that streams samples
/// in `CHUNK_SAMPLES`-sized blocks via `SoundDecoder::read_samples` and
/// pushes them into a bounded shared queue holding ~2 seconds of audio.
/// The rodio source pulls from that queue on the audio callback thread,
/// so the full clip is never resident in memory.
///
/// On Stop and on Drop the thread is signalled to exit and joined so
/// the decoder can be reused (Stop) or fully released (Drop).
pub struct SoundViewer {
    info: SoundInfo,
    name: String,
    format: SoundFormat,
    duration: Duration,
    /// Holds the decoder when not playing. Moved into the producer
    /// thread on Play, returned via the `JoinHandle` when the thread
    /// exits, then stashed back here (after a `reset`).
    decoder: Option<SoundDecoder>,
    /// Audio output, kept on the viewer so playback persists across UI
    /// frames. `None` if the system has no usable audio device.
    audio: Option<Audio>,
    /// Active producer thread + shared buffer. `None` when not playing.
    playback: Option<Playback>,
    /// Set when the decoder fails. Surfaced in the player area.
    decode_error: Option<String>,
}

struct Audio {
    /// Output device handle — must outlive `sink`.
    _stream: MixerDeviceSink,
    sink: Player,
}

impl SoundViewer {
    pub fn new(decoder: SoundDecoder) -> Self {
        let info = decoder.info();
        let name = decoder.name().to_string();
        let format = decoder.format();
        let duration = compute_duration(&info);
        let audio = init_audio();

        Self {
            info,
            name,
            format,
            duration,
            decoder: Some(decoder),
            audio,
            playback: None,
            decode_error: None,
        }
    }

    /// Spawn the producer thread and wire a streaming source into the
    /// rodio sink. Idempotent — does nothing if already playing.
    fn start_playback(&mut self) {
        if self.playback.is_some() {
            return;
        }
        let Some(audio) = self.audio.as_ref() else {
            return;
        };
        let Some(decoder) = self.decoder.take() else {
            return;
        };
        let Some(channels) = NonZero::new(self.info.channels) else {
            self.decoder = Some(decoder);
            return;
        };
        let Some(sample_rate) = NonZero::new(self.info.sample_rate) else {
            self.decoder = Some(decoder);
            return;
        };

        // 2 seconds of interleaved samples, with a small floor so very
        // low-rate fixtures still get a sensible buffer.
        let capacity = (2 * self.info.sample_rate as usize * self.info.channels as usize).max(8192);
        let buffer = Arc::new(AudioBuffer::new(capacity));

        let handle = {
            let buffer = Arc::clone(&buffer);
            thread::Builder::new()
                .name("sound-viewer-decoder".into())
                .spawn(move || decoder_loop(buffer, decoder))
                .expect("failed to spawn decoder thread")
        };

        let source = StreamingSource {
            buffer: Arc::clone(&buffer),
            channels,
            sample_rate,
            total_duration: self.duration,
        };

        // Append the source first (still paused), give the producer a
        // moment to fill some samples, then unpause — avoids silence at
        // the very start of playback.
        audio.sink.pause();
        audio.sink.append(source);
        for _ in 0..50 {
            if buffer.has_samples() {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        audio.sink.play();

        self.playback = Some(Playback {
            buffer,
            handle: Some(handle),
        });
    }

    /// Tear down the active playback: signal the producer thread to
    /// exit, drain the rodio sink, join the thread, and put the
    /// decoder back into `self.decoder` after rewinding it.
    fn stop_playback(&mut self) {
        let Some(mut playback) = self.playback.take() else {
            return;
        };
        playback.buffer.signal_stop();
        if let Some(audio) = self.audio.as_ref() {
            audio.sink.stop();
        }
        if let Some(handle) = playback.handle.take()
            && let Ok(mut decoder) = handle.join()
        {
            if let Err(e) = decoder.reset() {
                error!("decoder reset failed: {e}");
            }
            self.decoder = Some(decoder);
        }
    }

    /// Detect natural end-of-stream — buffer EOS flagged + sink drained
    /// — and reclaim the decoder so the next Play press starts clean.
    fn poll_for_eos(&mut self) {
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let sink_empty = self.audio.as_ref().map(|a| a.sink.empty()).unwrap_or(true);
        let thread_done = playback
            .handle
            .as_ref()
            .map(|h| h.is_finished())
            .unwrap_or(true);
        if sink_empty && thread_done {
            // Take the playback out so we can join the handle. `join`
            // here does not block — `is_finished` was true above.
            let mut playback = self.playback.take().unwrap();
            if let Some(handle) = playback.handle.take()
                && let Ok(mut decoder) = handle.join()
            {
                if let Err(e) = decoder.reset() {
                    error!("decoder reset failed: {e}");
                }
                self.decoder = Some(decoder);
            }
        }
    }
}

impl Drop for SoundViewer {
    fn drop(&mut self) {
        // Make sure the producer thread is fully gone before the
        // viewer's heap allocations go away.
        self.stop_playback();
    }
}

#[inline]
fn i16_to_f32(s: i16) -> f32 {
    s as f32 / 32768.0
}

fn compute_duration(info: &SoundInfo) -> Duration {
    if info.sample_rate == 0 || info.channels == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(info.frames() as f64 / info.sample_rate as f64)
}

fn init_audio() -> Option<Audio> {
    let mut device = DeviceSinkBuilder::open_default_sink().ok()?;
    // Avoid a noisy "device dropped while still streaming" log when the
    // viewer is replaced — the explorer flips between viewers regularly.
    device.log_on_drop(false);
    let sink = Player::connect_new(device.mixer());
    sink.pause(); // start paused; user presses Play to begin
    Some(Audio {
        _stream: device,
        sink,
    })
}

// ─── Streaming buffer + producer thread ───────────────────────────────────────

/// Bounded SPMC-style queue protected by a `Mutex` + `Condvar`. The
/// producer waits when the queue is full; the consumer (audio source)
/// never waits — it returns silence on underrun and `None` once the
/// producer has flagged end-of-stream.
struct AudioBuffer {
    queue: Mutex<VecDeque<f32>>,
    cond: Condvar,
    capacity: usize,
    /// Producer thread reached natural end-of-stream (or hit an error).
    eos: AtomicBool,
    /// UI thread (Stop or Drop) has asked the producer to exit promptly.
    stop: AtomicBool,
}

impl AudioBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            cond: Condvar::new(),
            capacity,
            eos: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        }
    }

    fn signal_stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.cond.notify_all();
    }

    fn has_samples(&self) -> bool {
        !self.queue.lock().unwrap().is_empty()
    }
}

/// Pull samples from `decoder` in `CHUNK_SAMPLES`-sized blocks and push
/// them into `buffer`. Returns the decoder so the UI can rewind it for
/// the next playback.
fn decoder_loop(buffer: Arc<AudioBuffer>, mut decoder: SoundDecoder) -> SoundDecoder {
    let mut chunk = vec![0i16; CHUNK_SAMPLES];
    loop {
        if buffer.stop.load(Ordering::Acquire) {
            return decoder;
        }

        // Park the thread until the queue has room for another chunk.
        {
            let mut q = buffer.queue.lock().unwrap();
            while q.len() + chunk.len() > buffer.capacity && !buffer.stop.load(Ordering::Acquire) {
                q = buffer.cond.wait(q).unwrap();
            }
            if buffer.stop.load(Ordering::Acquire) {
                return decoder;
            }
        }

        let n = match decoder.read_samples(&mut chunk) {
            Ok(0) => {
                buffer.eos.store(true, Ordering::Release);
                buffer.cond.notify_all();
                return decoder;
            }
            Ok(n) => n,
            Err(e) => {
                error!("[{}] decode error: {e}", decoder.name());
                buffer.eos.store(true, Ordering::Release);
                buffer.cond.notify_all();
                return decoder;
            }
        };

        let mut q = buffer.queue.lock().unwrap();
        for &s in &chunk[..n] {
            q.push_back(i16_to_f32(s));
        }
        buffer.cond.notify_all();
    }
}

/// Active playback state held by the viewer while the decoder thread
/// is alive.
struct Playback {
    buffer: Arc<AudioBuffer>,
    handle: Option<thread::JoinHandle<SoundDecoder>>,
}

/// rodio Source pulling samples out of [`AudioBuffer`].
struct StreamingSource {
    buffer: Arc<AudioBuffer>,
    channels: ChannelCount,
    sample_rate: SampleRate,
    total_duration: Duration,
}

impl Iterator for StreamingSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut q = self.buffer.queue.lock().unwrap();
        if let Some(s) = q.pop_front() {
            // Wake the producer in case it was parked on a full queue.
            self.buffer.cond.notify_all();
            return Some(s);
        }
        // Empty queue — was that "underrun" or "natural EOS"?
        if self.buffer.eos.load(Ordering::Acquire) {
            return None;
        }
        // Underrun. Briefly wait for samples instead of immediately
        // injecting silence, but never block the audio thread for long.
        let (mut q, _) = self
            .buffer
            .cond
            .wait_timeout(q, Duration::from_millis(5))
            .unwrap();
        if let Some(s) = q.pop_front() {
            self.buffer.cond.notify_all();
            return Some(s);
        }
        if self.buffer.eos.load(Ordering::Acquire) {
            return None;
        }
        Some(0.0)
    }
}

impl Source for StreamingSource {
    fn current_span_len(&self) -> Option<usize> {
        None
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

// ─── UI ───────────────────────────────────────────────────────────────────────

impl ResourceViewerTrait for SoundViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        // Reclaim the decoder if playback finished naturally between frames.
        self.poll_for_eos();

        // ── Bottom info bar ────────────────────────────────────────────────
        egui::Panel::bottom("sound_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.name);
                ui.separator();
                ui.label(self.format.to_string());
                ui.separator();
                ui.label(format!("{} Hz", self.info.sample_rate));
                ui.separator();
                ui.label(format!("{} ch", self.info.channels));
                ui.separator();
                ui.label(format!("{}-bit", self.info.bits_per_sample));
                ui.separator();
                ui.label(format!("{} samples", self.info.frames()));
                ui.separator();
                ui.label(format_duration(self.duration));
            });
        });

        // ── Central player ─────────────────────────────────────────────────
        // Snapshot read-only state up-front so the UI closure doesn't
        // need to borrow `self` immutably at the same time as the click
        // handlers borrow it mutably.
        let duration = self.duration;
        let has_decode_error = self.decode_error.is_some();
        let decode_error_msg = self.decode_error.clone();
        let audio_missing = self.audio.is_none();
        let pos = self.current_position();
        let is_playing = self.is_playing();
        let has_audio = self.decoder.is_some() || self.playback.is_some();

        let mut click_play_pause = false;
        let mut click_stop = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(24.0);
                    ui.heading("Sound Player");
                    ui.add_space(16.0);

                    if has_decode_error {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            decode_error_msg.as_deref().unwrap_or(""),
                        );
                        return;
                    }

                    if audio_missing {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "No audio output device available — playback disabled.",
                        );
                        return;
                    }

                    // ── Progress bar ──
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

                    // ── Transport buttons (single Play/Pause toggle + Stop) ──
                    ui.allocate_ui_with_layout(
                        egui::vec2(220.0, 0.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let label = if is_playing {
                                "⏸  Pause"
                            } else {
                                "▶  Play"
                            };
                            if ui
                                .add_enabled(has_audio, egui::Button::new(label))
                                .clicked()
                            {
                                click_play_pause = true;
                            }
                            if ui
                                .add_enabled(has_audio, egui::Button::new("⏹  Stop"))
                                .clicked()
                            {
                                click_stop = true;
                            }
                        },
                    );

                    if is_playing {
                        // Repaint while playing so the progress bar
                        // tracks playback in real time.
                        ui.ctx().request_repaint_after(Duration::from_millis(100));
                    }
                });
            });

        // Apply transport actions outside the UI closure so the methods
        // get exclusive access to `self`.
        if click_play_pause {
            self.toggle_play_pause();
        }
        if click_stop {
            self.stop_playback();
        }
    }
}

impl SoundViewer {
    fn is_playing(&self) -> bool {
        self.playback.is_some()
            && self
                .audio
                .as_ref()
                .map(|a| !a.sink.is_paused() && !a.sink.empty())
                .unwrap_or(false)
    }

    fn current_position(&self) -> Duration {
        match self.audio.as_ref() {
            Some(a) if !a.sink.empty() => a.sink.get_pos(),
            _ => Duration::ZERO,
        }
    }

    fn toggle_play_pause(&mut self) {
        let Some(audio) = self.audio.as_ref() else {
            return;
        };
        if self.playback.is_some() {
            // Already streaming — just toggle the sink. The producer
            // thread keeps filling the buffer (and parks on the
            // condvar when it fills up), so we don't tear it down.
            if audio.sink.is_paused() {
                audio.sink.play();
            } else {
                audio.sink.pause();
            }
        } else {
            // Cold start: spawn the producer + attach a fresh source.
            self.start_playback();
        }
    }
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs_f64();
    let mins = (total / 60.0).floor() as u64;
    let secs = total - (mins as f64) * 60.0;
    format!("{mins}:{:05.2}", secs)
}
