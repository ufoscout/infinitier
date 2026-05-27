//! Sound player. GPUI port of the egui `SoundViewer`.
//!
//! The decode + audio pipeline is reused verbatim from the egui
//! version (background producer thread + bounded shared queue +
//! `rodio::Source` consumer); only the UI bits change to use
//! `gpui-component` widgets and `window.request_animation_frame()`
//! for the playhead.

use std::collections::VecDeque;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, button::Button, h_flex, progress::Progress, v_flex,
};
use infinitier_core::{
    game::{GameResource, ResourceId},
    imported_resource::sound::{SoundDecoder, SoundFormat, SoundInfo},
};
use log::error;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

/// How many `i16` samples the decoder thread reads in one
/// `read_samples` call. A small chunk keeps lock-hold time short and
/// lets the producer react to a Stop request promptly.
const CHUNK_SAMPLES: usize = 2048;

pub struct SoundViewer {
    info: SoundInfo,
    name: String,
    format: SoundFormat,
    duration: Duration,
    /// Holds the decoder when not playing. Moved into the producer
    /// thread on Play, returned via the `JoinHandle` when the thread
    /// exits, then stashed back here (after a `reset`).
    decoder: Option<SoundDecoder>,
    /// Audio output, kept on the viewer so playback persists across
    /// UI frames. `None` if the system has no usable audio device.
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
        Self {
            info,
            name,
            format,
            duration,
            decoder: Some(decoder),
            audio: init_audio(),
            playback: None,
            decode_error: None,
        }
    }

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

        let capacity =
            (2 * self.info.sample_rate as usize * self.info.channels as usize).max(8192);
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
            if audio.sink.is_paused() {
                audio.sink.play();
            } else {
                audio.sink.pause();
            }
        } else {
            self.start_playback();
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

impl ResourceViewerTrait for SoundViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        // Reclaim the decoder if playback finished naturally between
        // frames so the next Play press starts clean.
        self.poll_for_eos();

        // Keep the window repainting while the sink is producing so
        // the progress bar tracks playback in real time.
        if self.is_playing() {
            window.request_animation_frame();
        }

        let border = cx.theme().border;

        let player = central_player(self, cx);
        let info = info_bar(self, cx);

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(player)
            .child(div().h_px().bg(border))
            .child(info)
            .into_any_element()
    }
}

/// Central player area — title, status / progress, transport buttons.
fn central_player(
    viewer: &SoundViewer,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let has_decode_error = viewer.decode_error.is_some();
    let decode_error_msg = viewer.decode_error.clone();
    let audio_missing = viewer.audio.is_none();
    let pos = viewer.current_position();
    let duration = viewer.duration;
    let is_playing = viewer.is_playing();
    let has_audio = viewer.decoder.is_some() || viewer.playback.is_some();

    let progress_pct = if duration.as_secs_f64() > 0.0 {
        (pos.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32 * 100.0
    } else {
        0.0
    };

    let mut col = v_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .gap_3()
        .items_center()
        .justify_center()
        .p_6()
        .child(
            div()
                .text_size(px(20.))
                .font_weight(gpui::FontWeight::BOLD)
                .child("Sound Player"),
        );

    if has_decode_error {
        col = col.child(
            div()
                .text_color(gpui::rgb(0xff5555))
                .child(decode_error_msg.unwrap_or_default()),
        );
        return col;
    }
    if audio_missing {
        col = col.child(
            div()
                .text_color(gpui::rgb(0xddaa00))
                .child("No audio output device available — playback disabled."),
        );
        return col;
    }

    col.child(
        div()
            .w(px(420.))
            .child(Progress::new().value(progress_pct).bg(theme.accent)),
    )
    .child(
        div()
            .text_color(theme.muted_foreground)
            .child(format!(
                "{} / {}",
                format_duration(pos),
                format_duration(duration)
            )),
    )
    .child(
        h_flex()
            .gap_2()
            .child({
                let label = if is_playing { "⏸  Pause" } else { "▶  Play" };
                Button::new("sound-play")
                    .label(label)
                    .small()
                    .disabled(!has_audio)
                    .on_click(cx.listener(|this, _, _, cx| {
                        sound_viewer_mut(this).toggle_play_pause();
                        cx.notify();
                    }))
            })
            .child(
                Button::new("sound-stop")
                    .label("⏹  Stop")
                    .small()
                    .disabled(!has_audio)
                    .on_click(cx.listener(|this, _, _, cx| {
                        sound_viewer_mut(this).stop_playback();
                        cx.notify();
                    })),
            ),
    )
}

/// Bottom info bar — same cells the egui viewer paints.
fn info_bar(viewer: &SoundViewer, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(cell(viewer.name.clone()))
        .child(separator(theme.border))
        .child(cell(viewer.format.to_string()))
        .child(separator(theme.border))
        .child(cell(format!("{} Hz", viewer.info.sample_rate)))
        .child(separator(theme.border))
        .child(cell(format!("{} ch", viewer.info.channels)))
        .child(separator(theme.border))
        .child(cell(format!("{}-bit", viewer.info.bits_per_sample)))
        .child(separator(theme.border))
        .child(cell(format!("{} samples", viewer.info.frames())))
        .child(separator(theme.border))
        .child(cell(format_duration(viewer.duration)))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}

fn sound_viewer_mut(app: &mut ExplorerApp) -> &mut SoundViewer {
    let trait_obj = &mut app
        .viewer
        .inner
        .as_mut()
        .expect("sound click fired without an active viewer")
        .viewer;
    (trait_obj.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<SoundViewer>()
        .expect("active viewer is not a SoundViewer")
}

// ─── Streaming buffer + producer thread (ported verbatim from egui) ──

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
    device.log_on_drop(false);
    let sink = Player::connect_new(device.mixer());
    sink.pause();
    Some(Audio {
        _stream: device,
        sink,
    })
}

struct AudioBuffer {
    queue: Mutex<VecDeque<f32>>,
    cond: Condvar,
    capacity: usize,
    eos: AtomicBool,
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

fn decoder_loop(buffer: Arc<AudioBuffer>, mut decoder: SoundDecoder) -> SoundDecoder {
    let mut chunk = vec![0i16; CHUNK_SAMPLES];
    loop {
        if buffer.stop.load(Ordering::Acquire) {
            return decoder;
        }
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

struct Playback {
    buffer: Arc<AudioBuffer>,
    handle: Option<thread::JoinHandle<SoundDecoder>>,
}

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
            self.buffer.cond.notify_all();
            return Some(s);
        }
        if self.buffer.eos.load(Ordering::Acquire) {
            return None;
        }
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

fn format_duration(d: Duration) -> String {
    let total = d.as_secs_f64();
    let mins = (total / 60.0).floor() as u64;
    let secs = total - (mins as f64) * 60.0;
    format!("{mins}:{:05.2}", secs)
}
