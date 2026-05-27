//! Movie viewer with streaming playback — direct port of the egui
//! original. A producer thread pre-buffers ~3 s of video frames and
//! audio samples; the Slint Timer (see `app::run`) pumps due video
//! frames into the `image` property and the rodio sink consumes audio
//! on its own callback thread.

use std::collections::VecDeque;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use infinitier_core::imported_resource::movie::{
    MovieDecoder, MovieFormat, MovieSource, MovieVideoFrame,
};
use log::error;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::MainWindow;
use crate::state::AppState;

const PREBUFFER_US: u64 = 3_000_000;
const AUDIO_CAPACITY: usize = 3 * 48_000 * 2;
const AUDIO_FMT_TIMEOUT_MS: u64 = 250;

pub fn populate(window: &MainWindow, state: &Rc<AppState>, src: MovieSource) {
    let viewer = MovieViewerState::new(src);
    *state.movie_viewer.borrow_mut() = Some(viewer);

    window.set_viewer_kind("movie".into());
    refresh(window, state);
}

pub fn refresh(window: &MainWindow, state: &Rc<AppState>) {
    let guard = state.movie_viewer.borrow();
    let Some(mv) = guard.as_ref() else { return };

    window.set_movie_name(mv.source.name.clone().into());
    window.set_movie_format(
        mv.format
            .as_ref()
            .map(|f| f.to_string())
            .unwrap_or_default()
            .into(),
    );
    window.set_movie_dims(format!("{} × {}", mv.width, mv.height).into());
    let fps = if mv.frame_duration_us > 0 {
        1_000_000.0 / mv.frame_duration_us as f64
    } else {
        0.0
    };
    window.set_movie_fps(format!("{fps:.2} fps").into());
    let pos = mv.playback_position();
    let progress = if mv.total_duration.as_secs_f64() > 0.0 {
        (pos.as_secs_f64() / mv.total_duration.as_secs_f64()).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    window.set_movie_progress(progress);
    window.set_movie_position_text(format_duration(pos).into());
    window.set_movie_duration_text(format_duration(mv.total_duration).into());
    window.set_movie_is_playing(mv.is_playing());
    window.set_movie_has_movie(mv.frame_duration_us > 0 && mv.decode_error.is_none());
    window.set_movie_decode_error(mv.decode_error.clone().unwrap_or_default().into());
    window.set_movie_bitmap(mv.last_image.clone().unwrap_or_default());
}

/// Per-tick maintenance: pull due frames, detect natural EOS, refresh
/// every progress-style property.
pub fn tick(window: &MainWindow, state: &Rc<AppState>) {
    {
        let mut guard = state.movie_viewer.borrow_mut();
        if let Some(mv) = guard.as_mut() {
            mv.advance_frames();
            mv.poll_for_eos();
        } else {
            return;
        }
    }
    refresh(window, state);
}

pub fn on_play_pause_clicked(window: &MainWindow, state: &Rc<AppState>) {
    if let Some(mv) = state.movie_viewer.borrow_mut().as_mut() {
        if mv.playback.is_none() {
            mv.start_playback();
        } else if mv.is_paused() {
            mv.resume();
        } else {
            mv.pause();
        }
    }
    refresh(window, state);
}

pub fn on_stop_clicked(window: &MainWindow, state: &Rc<AppState>) {
    if let Some(mv) = state.movie_viewer.borrow_mut().as_mut() {
        mv.stop_playback();
    }
    refresh(window, state);
}

// ── State ─────────────────────────────────────────────────────────────────────

pub struct MovieViewerState {
    source: MovieSource,
    format: Option<MovieFormat>,
    width: u16,
    height: u16,
    frame_duration_us: u32,
    total_duration: Duration,
    decode_error: Option<String>,

    audio: Option<Audio>,
    playback: Option<Playback>,
    /// Latest decoded frame uploaded to a `slint::Image`, ready to
    /// push into the `movie-bitmap` property.
    last_image: Option<Image>,
}

struct Audio {
    _stream: MixerDeviceSink,
    sink: Player,
}

impl MovieViewerState {
    fn new(source: MovieSource) -> Self {
        let (format, width, height, frame_duration_us, total_duration, decode_error) =
            match source.open() {
                Ok(dec) => {
                    let info = dec.info();
                    (
                        Some(dec.format()),
                        info.width,
                        info.height,
                        info.frame_duration_us,
                        info.total_duration_us,
                        None,
                    )
                }
                Err(e) => (None, 0, 0, 0, 0, Some(format!("failed to open movie: {e}"))),
            };

        Self {
            source,
            format,
            width,
            height,
            frame_duration_us,
            total_duration: Duration::from_micros(total_duration),
            decode_error,
            audio: init_audio(),
            playback: None,
            last_image: None,
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
                self.decode_error = Some(format!("failed to open movie: {e}"));
                return;
            }
        };

        let video_capacity = ((PREBUFFER_US / self.frame_duration_us as u64) as usize).max(8);
        let state = Arc::new(PlaybackState::new(AUDIO_CAPACITY, video_capacity));

        let handle = {
            let state = Arc::clone(&state);
            thread::Builder::new()
                .name("movie-viewer-decoder".into())
                .spawn(move || decoder_loop(state, decoder))
                .expect("failed to spawn movie decoder thread")
        };

        if let Some(audio) = self.audio.as_ref()
            && let Some((channels, sample_rate)) =
                state.wait_for_audio_fmt(Duration::from_millis(AUDIO_FMT_TIMEOUT_MS))
        {
            audio.sink.pause();
            audio.sink.append(MovieAudioSource {
                state: Arc::clone(&state),
                channels,
                sample_rate,
            });
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
            error!("movie decoder thread panicked: {e:?}");
        }
        self.last_image = None;
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

    fn advance_frames(&mut self) {
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let frame_dur_us = self.frame_duration_us as u64;
        if frame_dur_us == 0 {
            return;
        }

        let now = Instant::now();
        let mut elapsed = now
            .duration_since(playback.epoch)
            .saturating_sub(playback.paused_for);
        if let Some(pause_started) = playback.pause_started {
            elapsed = elapsed.saturating_sub(now.duration_since(pause_started));
        }
        let target = elapsed.as_micros() as u64 / frame_dur_us;

        let mut latest: Option<MovieVideoFrame> = None;
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
            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                &frame.pixels,
                frame.width as u32,
                frame.height as u32,
            );
            self.last_image = Some(Image::from_rgba8(buffer));
        }
    }

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

impl Drop for MovieViewerState {
    fn drop(&mut self) {
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

// ── Producer thread + shared state ────────────────────────────────────────────

struct Playback {
    state: Arc<PlaybackState>,
    handle: Option<thread::JoinHandle<()>>,
    epoch: Instant,
    paused_for: Duration,
    pause_started: Option<Instant>,
    frames_consumed: u64,
}

struct PlaybackState {
    audio_queue: Mutex<VecDeque<f32>>,
    audio_cond: Condvar,
    audio_capacity: usize,

    video_queue: Mutex<VecDeque<MovieVideoFrame>>,
    video_cond: Condvar,
    video_capacity: usize,

    audio_fmt: Mutex<Option<(NonZero<u16>, NonZero<u32>)>>,
    audio_fmt_cond: Condvar,

    eos: AtomicBool,
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
        self.audio_cond.notify_all();
        self.video_cond.notify_all();
        self.audio_fmt_cond.notify_all();
    }

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

fn decoder_loop(state: Arc<PlaybackState>, mut decoder: MovieDecoder) {
    loop {
        if state.stop.load(Ordering::Acquire) {
            return;
        }

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
                error!("[movie] decode error: {e}");
                state.eos.store(true, Ordering::Release);
                state.audio_cond.notify_all();
                state.video_cond.notify_all();
                state.audio_fmt_cond.notify_all();
                return;
            }
        };

        for chunk in &frame.audio {
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

// ── rodio Source pulling audio out of the playback state ──────────────────────

struct MovieAudioSource {
    state: Arc<PlaybackState>,
    channels: ChannelCount,
    sample_rate: SampleRate,
}

impl Iterator for MovieAudioSource {
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

impl Source for MovieAudioSource {
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

fn format_duration(d: Duration) -> String {
    let total = d.as_secs_f64();
    let mins = (total / 60.0).floor() as u64;
    let secs = total - (mins as f64) * 60.0;
    format!("{mins}:{:05.2}", secs)
}
