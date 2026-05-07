use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    movie::{MovieSource, StreamingMveDecoder},
    resource::mve::VideoFrame,
};
use log::error;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use std::collections::VecDeque;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Target prebuffer for the producer thread, in microseconds. Three
/// seconds of video frames + audio samples are kept in front of the
/// consumer at all times.
const PREBUFFER_US: u64 = 3_000_000;

/// Worst-case audio capacity (3 s @ 48 kHz stereo). Allocated up front;
/// the actual sample rate is detected from the first audio chunk.
const AUDIO_CAPACITY: usize = 3 * 48_000 * 2;

/// How long to wait for the producer thread to surface the file's
/// audio format (channels + sample rate) before giving up and playing
/// without sound. MVE files in the wild always emit audio metadata in
/// the first frame, so 250 ms is plenty.
const AUDIO_FMT_TIMEOUT_MS: u64 = 250;

/// Streaming MVE player.
///
/// On Play, the viewer:
/// 1. Opens a fresh [`StreamingMveDecoder`] from the saved
///    [`MovieSource`].
/// 2. Spawns a producer thread that feeds two queues — interleaved
///    `f32` audio samples and decoded RGBA video frames — pre-buffered
///    to ~3 seconds.
/// 3. Attaches a [`MveStreamSource`] to the rodio sink which pulls
///    audio from the audio queue, and on every UI tick the central
///    panel pulls the next due frame from the video queue.
///
/// Stop signals the thread, drains the rodio sink, joins the thread
/// (so the decoder is fully released), and clears the texture. The
/// `Drop` impl does the same so threads don't survive viewer changes.
pub struct MveViewer {
    source: MovieSource,
    width: u16,
    height: u16,
    frame_duration_us: u32,
    /// Fallback for the bottom info bar when the file can't be opened
    /// for metadata.
    decode_error: Option<String>,

    audio: Option<Audio>,
    playback: Option<Playback>,
    /// Texture that holds the most recently shown video frame. Reused
    /// across frames so we don't allocate a new GPU resource every tick.
    texture: Option<egui::TextureHandle>,
}

struct Audio {
    _stream: MixerDeviceSink,
    sink: Player,
}

impl MveViewer {
    pub fn new(source: MovieSource) -> Self {
        // Open once just to extract metadata for the info bar. Drop the
        // decoder immediately — actual playback opens a fresh one.
        //
        // `MveDecoder::new` only consumes the two init chunks (which
        // give us width/height); the timer chunk lives inside the first
        // frame, so `frame_duration_us` is still 0 right after `new`.
        // Pull one frame here to surface the timer.
        let (width, height, frame_duration_us, decode_error) = match source.open() {
            Ok(mut dec) => {
                let _ = dec.next_frame();
                (dec.width(), dec.height(), dec.frame_duration_us(), None)
            }
            Err(e) => (0, 0, 0, Some(format!("failed to open MVE: {e}"))),
        };

        Self {
            source,
            width,
            height,
            frame_duration_us,
            decode_error,
            audio: init_audio(),
            playback: None,
            texture: None,
        }
    }

    fn start_playback(&mut self) {
        if self.playback.is_some() {
            return;
        }
        if self.frame_duration_us == 0 {
            return;
        }

        let decoder = match self.source.open() {
            Ok(d) => d,
            Err(e) => {
                self.decode_error = Some(format!("failed to open MVE: {e}"));
                return;
            }
        };

        // Three seconds worth of video frames, with a small floor so
        // very short clips still buffer ahead.
        let video_capacity = ((PREBUFFER_US / self.frame_duration_us as u64) as usize).max(8);

        let state = Arc::new(PlaybackState::new(AUDIO_CAPACITY, video_capacity));

        let handle = {
            let state = Arc::clone(&state);
            thread::Builder::new()
                .name("mve-viewer-decoder".into())
                .spawn(move || decoder_loop(state, decoder))
                .expect("failed to spawn mve decoder thread")
        };

        // Wire audio if the producer surfaces a format quickly. If it
        // doesn't (no audio in the first frames, or stop arrived early),
        // fall through and play silent video.
        if let Some(audio) = self.audio.as_ref()
            && let Some((channels, sample_rate)) =
                state.wait_for_audio_fmt(Duration::from_millis(AUDIO_FMT_TIMEOUT_MS))
        {
            audio.sink.pause();
            audio.sink.append(MveStreamSource {
                state: Arc::clone(&state),
                channels,
                sample_rate,
            });
            // Brief warm-up so the very first samples aren't silence.
            for _ in 0..50 {
                if !state.audio_queue.lock().unwrap().is_empty() {
                    break;
                }
                thread::sleep(Duration::from_millis(2));
            }
            audio.sink.play();
        }

        self.playback = Some(Playback {
            state,
            handle: Some(handle),
            epoch: Instant::now(),
            paused_for: Duration::ZERO,
            pause_started: None,
            frames_consumed: 0,
        });
    }

    fn stop_playback(&mut self) {
        let Some(mut playback) = self.playback.take() else {
            return;
        };
        playback.state.signal_stop();
        if let Some(audio) = self.audio.as_ref() {
            audio.sink.stop();
        }
        if let Some(handle) = playback.handle.take()
            && let Err(e) = handle.join()
        {
            error!("mve decoder thread panicked: {e:?}");
        }
        self.texture = None;
    }

    fn pause(&mut self) {
        if let Some(playback) = self.playback.as_mut()
            && playback.pause_started.is_none()
        {
            playback.pause_started = Some(Instant::now());
        }
        if let Some(audio) = self.audio.as_ref() {
            audio.sink.pause();
        }
    }

    fn resume(&mut self) {
        if let Some(playback) = self.playback.as_mut()
            && let Some(pause_started) = playback.pause_started.take()
        {
            playback.paused_for += Instant::now().duration_since(pause_started);
        }
        if let Some(audio) = self.audio.as_ref() {
            audio.sink.play();
        }
    }

    fn is_paused(&self) -> bool {
        self.playback
            .as_ref()
            .map(|p| p.pause_started.is_some())
            .unwrap_or(false)
    }

    fn is_playing(&self) -> bool {
        self.playback.is_some() && !self.is_paused()
    }

    fn playback_position(&self) -> Duration {
        let Some(playback) = self.playback.as_ref() else {
            return Duration::ZERO;
        };
        let now = Instant::now();
        let mut elapsed = now
            .duration_since(playback.epoch)
            .saturating_sub(playback.paused_for);
        if let Some(pause_started) = playback.pause_started {
            elapsed = elapsed.saturating_sub(now.duration_since(pause_started));
        }
        elapsed
    }

    /// Pop video frames whose presentation time has passed and upload
    /// the latest one (we may pop several if the UI was lagging) into
    /// the GPU texture.
    fn advance_frames(&mut self, ctx: &egui::Context) {
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let frame_dur_us = self.frame_duration_us as u64;
        if frame_dur_us == 0 {
            return;
        }

        // Inline `playback_position` here so we can keep the `&mut
        // Playback` borrow alive while we compute it.
        let now = Instant::now();
        let mut elapsed = now
            .duration_since(playback.epoch)
            .saturating_sub(playback.paused_for);
        if let Some(pause_started) = playback.pause_started {
            elapsed = elapsed.saturating_sub(now.duration_since(pause_started));
        }
        let target = elapsed.as_micros() as u64 / frame_dur_us;

        let mut latest: Option<VideoFrame> = None;
        {
            let mut q = playback.state.video_queue.lock().unwrap();
            while playback.frames_consumed <= target {
                match q.pop_front() {
                    Some(frame) => {
                        latest = Some(frame);
                        playback.frames_consumed += 1;
                    }
                    None => break,
                }
            }
        }
        playback.state.video_cond.notify_all();

        if let Some(frame) = latest {
            let img = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.pixels,
            );
            match self.texture.as_mut() {
                Some(handle) => handle.set(img, egui::TextureOptions::LINEAR),
                None => {
                    self.texture =
                        Some(ctx.load_texture("mve-frame", img, egui::TextureOptions::LINEAR));
                }
            }
        }
    }

    /// Detect natural EOS: producer has flagged it AND both queues are
    /// drained, AND audio sink (if any) is empty. Then teardown.
    fn poll_for_eos(&mut self) {
        let Some(playback) = self.playback.as_ref() else {
            return;
        };
        let producer_done = playback.state.eos.load(Ordering::Acquire);
        if !producer_done {
            return;
        }
        let video_drained = playback.state.video_queue.lock().unwrap().is_empty();
        let audio_drained = self.audio.as_ref().map(|a| a.sink.empty()).unwrap_or(true);
        if video_drained && audio_drained {
            self.stop_playback();
        }
    }
}

impl Drop for MveViewer {
    fn drop(&mut self) {
        // Make sure the producer thread is fully gone before the
        // viewer's heap allocations go away.
        self.stop_playback();
    }
}

fn init_audio() -> Option<Audio> {
    let mut device = DeviceSinkBuilder::open_default_sink().ok()?;
    device.log_on_drop(false);
    let sink = Player::connect_new(device.mixer());
    sink.pause();
    Some(Audio {
        _stream: device,
        sink,
    })
}

// ─── Producer thread + shared state ───────────────────────────────────────────

/// Active playback session. Dropped on Stop and on Drop of the viewer.
struct Playback {
    state: Arc<PlaybackState>,
    handle: Option<thread::JoinHandle<()>>,
    /// Wall-clock instant that playback (re)started at.
    epoch: Instant,
    /// Total time spent paused in this session, accumulated each Resume.
    paused_for: Duration,
    /// If currently paused, when the pause began. `None` when running.
    pause_started: Option<Instant>,
    /// How many video frames the UI has popped from the queue. Used to
    /// decide which frames are still due.
    frames_consumed: u64,
}

/// Bounded shared state between the producer thread and the UI / audio
/// callback thread.
struct PlaybackState {
    audio_queue: Mutex<VecDeque<f32>>,
    audio_cond: Condvar,
    audio_capacity: usize,

    video_queue: Mutex<VecDeque<VideoFrame>>,
    video_cond: Condvar,
    video_capacity: usize,

    /// Audio format (channels, sample_rate) extracted from the first
    /// audio chunk. `None` until the producer sees one — the UI waits
    /// briefly on `audio_fmt_cond` so the rodio source is constructed
    /// with the right rate.
    audio_fmt: Mutex<Option<(NonZero<u16>, NonZero<u32>)>>,
    audio_fmt_cond: Condvar,

    /// Producer reached natural end-of-stream (or hit an error).
    eos: AtomicBool,
    /// UI thread asked the producer to exit ASAP.
    stop: AtomicBool,
}

impl PlaybackState {
    fn new(audio_capacity: usize, video_capacity: usize) -> Self {
        Self {
            audio_queue: Mutex::new(VecDeque::with_capacity(audio_capacity)),
            audio_cond: Condvar::new(),
            audio_capacity,
            video_queue: Mutex::new(VecDeque::with_capacity(video_capacity)),
            video_cond: Condvar::new(),
            video_capacity,
            audio_fmt: Mutex::new(None),
            audio_fmt_cond: Condvar::new(),
            eos: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        }
    }

    fn signal_stop(&self) {
        self.stop.store(true, Ordering::Release);
        // Wake every parked waiter (producer parked on a full queue,
        // UI parked on the audio fmt).
        self.audio_cond.notify_all();
        self.video_cond.notify_all();
        self.audio_fmt_cond.notify_all();
    }

    /// Wait up to `timeout` for the producer to emit an audio format.
    /// Returns `None` on timeout / EOS-without-audio / stop.
    fn wait_for_audio_fmt(&self, timeout: Duration) -> Option<(NonZero<u16>, NonZero<u32>)> {
        let q = self.audio_fmt.lock().unwrap();
        let (q, _) = self
            .audio_fmt_cond
            .wait_timeout_while(q, timeout, |fmt| {
                fmt.is_none()
                    && !self.eos.load(Ordering::Acquire)
                    && !self.stop.load(Ordering::Acquire)
            })
            .unwrap();
        *q
    }
}

fn decoder_loop(state: Arc<PlaybackState>, mut decoder: StreamingMveDecoder) {
    loop {
        if state.stop.load(Ordering::Acquire) {
            return;
        }

        // Park until there's room for another video frame. The audio
        // queue's own backpressure is handled inside the audio push
        // below.
        {
            let mut q = state.video_queue.lock().unwrap();
            while q.len() >= state.video_capacity && !state.stop.load(Ordering::Acquire) {
                q = state.video_cond.wait(q).unwrap();
            }
            if state.stop.load(Ordering::Acquire) {
                return;
            }
        }

        let frame = match decoder.next_frame() {
            Ok(Some(f)) => f,
            Ok(None) => {
                state.eos.store(true, Ordering::Release);
                state.audio_cond.notify_all();
                state.video_cond.notify_all();
                state.audio_fmt_cond.notify_all();
                return;
            }
            Err(e) => {
                error!("[mve] decode error: {e}");
                state.eos.store(true, Ordering::Release);
                state.audio_cond.notify_all();
                state.video_cond.notify_all();
                state.audio_fmt_cond.notify_all();
                return;
            }
        };

        for chunk in &frame.audio {
            // Surface the audio format the first time we see one.
            {
                let mut fmt = state.audio_fmt.lock().unwrap();
                if fmt.is_none()
                    && let Some(c) = NonZero::new(chunk.channels as u16)
                    && let Some(r) = NonZero::new(chunk.sample_rate)
                {
                    *fmt = Some((c, r));
                    state.audio_fmt_cond.notify_all();
                }
            }

            let mut q = state.audio_queue.lock().unwrap();
            while q.len() + chunk.samples.len() > state.audio_capacity
                && !state.stop.load(Ordering::Acquire)
            {
                q = state.audio_cond.wait(q).unwrap();
            }
            if state.stop.load(Ordering::Acquire) {
                return;
            }
            for &s in &chunk.samples {
                q.push_back(s as f32 / 32768.0);
            }
            state.audio_cond.notify_all();
        }

        let mut q = state.video_queue.lock().unwrap();
        q.push_back(frame.video);
        state.video_cond.notify_all();
    }
}

// ─── rodio Source pulling audio out of the playback state ─────────────────────

struct MveStreamSource {
    state: Arc<PlaybackState>,
    channels: ChannelCount,
    sample_rate: SampleRate,
}

impl Iterator for MveStreamSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let q = self.state.audio_queue.lock().unwrap();
        let mut q = q;
        if let Some(s) = q.pop_front() {
            self.state.audio_cond.notify_all();
            return Some(s);
        }
        if self.state.eos.load(Ordering::Acquire) {
            return None;
        }
        // Underrun: brief wait so we don't immediately inject silence.
        let (mut q, _) = self
            .state
            .audio_cond
            .wait_timeout(q, Duration::from_millis(5))
            .unwrap();
        if let Some(s) = q.pop_front() {
            self.state.audio_cond.notify_all();
            return Some(s);
        }
        if self.state.eos.load(Ordering::Acquire) {
            return None;
        }
        Some(0.0)
    }
}

impl Source for MveStreamSource {
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
        None
    }
}

// ─── UI ───────────────────────────────────────────────────────────────────────

impl ResourceViewerTrait for MveViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        // Pull due video frames every tick.
        self.advance_frames(ui.ctx());
        // Detect natural end-of-stream and tear down.
        self.poll_for_eos();

        // ── Bottom info bar ────────────────────────────────────────────────
        egui::Panel::bottom("mve_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.source.name);
                ui.separator();
                ui.label(format!("{}×{}", self.width, self.height));
                ui.separator();
                let fps = if self.frame_duration_us > 0 {
                    1_000_000.0 / self.frame_duration_us as f64
                } else {
                    0.0
                };
                ui.label(format!("{fps:.2} fps"));
                ui.separator();
                ui.label(format_duration(self.playback_position()));
            });
        });

        // ── Transport bar (above the info bar) ─────────────────────────────
        let is_playing = self.is_playing();
        let has_movie = self.frame_duration_us > 0 && self.decode_error.is_none();
        let pos = self.playback_position();

        let mut click_play_pause = false;
        let mut click_stop = false;

        egui::Panel::bottom("mve_transport_panel").show_inside(ui, |ui| {
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
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
                            .add_enabled(has_movie, egui::Button::new(label))
                            .clicked()
                        {
                            click_play_pause = true;
                        }
                        if ui
                            .add_enabled(has_movie, egui::Button::new("⏹  Stop"))
                            .clicked()
                        {
                            click_stop = true;
                        }
                    },
                );
                ui.add_space(6.0);
                // Progress bar below the buttons. We don't know the
                // total duration of an MVE without scanning the whole
                // file, so the bar shows elapsed time and a generous
                // best-effort fraction (capped so it doesn't run off
                // the end while playing).
                let elapsed_secs = pos.as_secs_f64();
                let progress = (elapsed_secs / 60.0).clamp(0.0, 1.0) as f32;
                ui.add(
                    egui::ProgressBar::new(progress)
                        .desired_width(420.0)
                        .text(format_duration(pos)),
                );
                ui.add_space(4.0);
            });
        });

        // ── Central area: video frame ──────────────────────────────────────
        let texture = self.texture.as_ref().cloned();
        let decode_error = self.decode_error.clone();
        let width = self.width;
        let height = self.height;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if let Some(err) = decode_error.as_deref() {
                        ui.colored_label(egui::Color32::LIGHT_RED, err);
                        return;
                    }
                    if let Some(tex) = texture.as_ref() {
                        // Fit-to-area while preserving aspect ratio.
                        let avail = ui.available_size();
                        let scale = (avail.x / width.max(1) as f32)
                            .min(avail.y / height.max(1) as f32)
                            .max(0.0);
                        let display_size = egui::vec2(width as f32 * scale, height as f32 * scale);
                        ui.add(egui::Image::new(tex).fit_to_exact_size(display_size));
                    } else {
                        ui.heading("Movie Player");
                        ui.add_space(12.0);
                        ui.label(format!("{width}×{height}"));
                    }
                });
            });

        // Apply transport actions outside the UI closures.
        if click_play_pause {
            if self.playback.is_none() {
                self.start_playback();
            } else if self.is_paused() {
                self.resume();
            } else {
                self.pause();
            }
        }
        if click_stop {
            self.stop_playback();
        }

        // While playing (or paused but with frames being slung), keep
        // repainting so the video and the progress bar update.
        if self.playback.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(
                (self.frame_duration_us as u64 / 1_000).max(16),
            ));
        }
    }
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs_f64();
    let mins = (total / 60.0).floor() as u64;
    let secs = total - (mins as f64) * 60.0;
    format!("{mins}:{:05.2}", secs)
}
