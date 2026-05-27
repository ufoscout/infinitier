//! Sound viewer with streaming playback — direct port of the egui
//! original's player. Decoding runs on a dedicated background thread
//! that pushes PCM samples into a bounded queue; the rodio sink pulls
//! from the queue on the audio callback thread.
//!
//! The Slint side has no `request_repaint_after` equivalent, so the
//! "progress bar advances while playing" effect is driven by a
//! `slint::Timer` (see `app::run`) that calls [`tick`] on every frame.

use std::collections::VecDeque;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use infinitier_core::game::GameResource;
use infinitier_core::imported_resource::sound::{SoundDecoder, SoundFormat, SoundInfo};
use log::error;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};

use crate::MainWindow;
use crate::state::AppState;

/// How many `i16` samples the decoder thread reads in one `read_samples`
/// call. A small chunk keeps lock-hold time short and lets the producer
/// react to a Stop request promptly.
const CHUNK_SAMPLES: usize = 2048;

/// Hands the viewer dispatcher a fresh state object for the selected
/// sound resource, then pushes the initial property snapshot to the
/// window.
pub fn populate(
    window: &MainWindow,
    state: &Rc<AppState>,
    decoder: SoundDecoder,
    _resource: &GameResource,
) {
    let viewer = SoundViewerState::new(decoder);
    *state.sound_viewer.borrow_mut() = Some(viewer);

    window.set_viewer_kind("sound".into());
    refresh(window, state);
}

/// Re-derive every sound-viewer property from `AppState::sound_viewer`.
pub fn refresh(window: &MainWindow, state: &Rc<AppState>) {
    let guard = state.sound_viewer.borrow();
    let Some(sv) = guard.as_ref() else { return };

    window.set_sound_name(sv.name.clone().into());
    window.set_sound_format(sv.format.to_string().into());
    window.set_sound_info(
        format!(
            "{} Hz · {} ch · {}-bit · {} samples",
            sv.info.sample_rate,
            sv.info.channels,
            sv.info.bits_per_sample,
            sv.info.frames(),
        )
        .into(),
    );
    let pos = sv.current_position();
    let progress = if sv.duration.as_secs_f64() > 0.0 {
        (pos.as_secs_f64() / sv.duration.as_secs_f64()).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    window.set_sound_progress(progress);
    window.set_sound_position_text(format_duration(pos).into());
    window.set_sound_duration_text(format_duration(sv.duration).into());
    window.set_sound_is_playing(sv.is_playing());
    window.set_sound_has_audio(sv.decoder.is_some() || sv.playback.is_some());
    window.set_sound_audio_missing(sv.audio.is_none());
    window.set_sound_decode_error(sv.decode_error.clone().unwrap_or_default().into());
}

/// Slint Timer tick. Polls for natural end-of-stream and pushes the
/// current playback position so the progress bar moves.
pub fn tick(window: &MainWindow, state: &Rc<AppState>) {
    {
        let mut guard = state.sound_viewer.borrow_mut();
        if let Some(sv) = guard.as_mut() {
            sv.poll_for_eos();
        } else {
            return;
        }
    }
    refresh(window, state);
}

pub fn on_play_pause_clicked(window: &MainWindow, state: &Rc<AppState>) {
    if let Some(sv) = state.sound_viewer.borrow_mut().as_mut() {
        sv.toggle_play_pause();
    }
    refresh(window, state);
}

pub fn on_stop_clicked(window: &MainWindow, state: &Rc<AppState>) {
    if let Some(sv) = state.sound_viewer.borrow_mut().as_mut() {
        sv.stop_playback();
    }
    refresh(window, state);
}

// ── State ─────────────────────────────────────────────────────────────────────

pub struct SoundViewerState {
    info: SoundInfo,
    name: String,
    format: SoundFormat,
    duration: Duration,
    decoder: Option<SoundDecoder>,
    audio: Option<Audio>,
    playback: Option<Playback>,
    decode_error: Option<String>,
}

struct Audio {
    _stream: MixerDeviceSink,
    sink: Player,
}

impl SoundViewerState {
    fn new(decoder: SoundDecoder) -> Self {
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

impl Drop for SoundViewerState {
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
    device.log_on_drop(false);
    let sink = Player::connect_new(device.mixer());
    sink.pause();
    Some(Audio {
        _stream: device,
        sink,
    })
}

// ── Streaming buffer + producer thread ────────────────────────────────────────

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
