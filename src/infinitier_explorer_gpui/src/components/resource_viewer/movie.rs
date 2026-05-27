//! Streaming movie player (MVE / WBM). GPUI port of the egui
//! `MovieViewer`.
//!
//! Decoding pipeline is identical to the egui version: a background
//! thread feeds two bounded queues (interleaved `f32` audio samples
//! and decoded RGBA video frames) pre-buffered ~3 s; a
//! `rodio::Source` pulls audio; the UI pulls due video frames on
//! every paint. The differences from the egui port are at the
//! presentation edge:
//!
//! - Video frames land in `Option<Arc<RenderImage>>` (gpui's image
//!   primitive) instead of `egui::TextureHandle::set`. We pre-swap
//!   RGBA → BGRA in place, same trick the image / bam viewers use.
//! - Repaints come from `window.request_animation_frame()` instead
//!   of `ctx.request_repaint_after`.
//! - Transport buttons + progress bar come from `gpui-component`.

use std::collections::VecDeque;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, IntoElement, ObjectFit, ParentElement, RenderImage, StyledImage as _,
    Styled, Window, div, img,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, button::Button, h_flex, progress::Progress, v_flex,
};
use image::{Frame, ImageBuffer, Rgba};
use infinitier_core::{
    game::{GameResource, ResourceId},
    imported_resource::movie::{MovieDecoder, MovieFormat, MovieSource, MovieVideoFrame},
};
use log::error;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use smallvec::SmallVec;

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

const PREBUFFER_US: u64 = 3_000_000;
const AUDIO_CAPACITY: usize = 3 * 48_000 * 2;
const AUDIO_FMT_TIMEOUT_MS: u64 = 250;

pub struct MovieViewer {
    source: MovieSource,
    format: Option<MovieFormat>,
    width: u16,
    height: u16,
    frame_duration_us: u32,
    total_duration: Duration,
    decode_error: Option<String>,

    audio: Option<Audio>,
    playback: Option<Playback>,
    /// Most recently uploaded video frame. Reused across frames by
    /// rebuilding a fresh `Arc<RenderImage>` whenever a new
    /// `MovieVideoFrame` is pulled from the producer queue.
    current_frame: Option<Arc<RenderImage>>,
}

struct Audio {
    _stream: MixerDeviceSink,
    sink: Player,
}

impl MovieViewer {
    pub fn new(source: MovieSource) -> Self {
        // Open once just to extract metadata for the info bar. Drop
        // the decoder immediately — actual playback opens a fresh one.
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
            current_frame: None,
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
        self.current_frame = None;
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

    /// Pop any video frames whose presentation time has passed and
    /// upload the latest one (we may pop several if the UI was
    /// lagging) into a fresh `Arc<RenderImage>` for the next paint.
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
            self.current_frame = Some(frame_to_render_image(frame));
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

impl Drop for MovieViewer {
    fn drop(&mut self) {
        self.stop_playback();
    }
}

impl ResourceViewerTrait for MovieViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        // Pull due video frames every tick, then detect natural EOS.
        self.advance_frames();
        self.poll_for_eos();

        // While playing (or paused but with frames being slung), keep
        // repainting so the video and the progress bar update.
        if self.playback.is_some() {
            window.request_animation_frame();
        }

        let border = cx.theme().border;

        let picture = picture_area(self.current_frame.clone(), &self.decode_error);
        let transport = transport_bar(self, cx);
        let info = info_bar(self, cx);

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(picture)
            .child(div().h_px().bg(border))
            .child(transport)
            .child(div().h_px().bg(border))
            .child(info)
            .into_any_element()
    }
}

fn picture_area(
    image: Option<Arc<RenderImage>>,
    decode_error: &Option<String>,
) -> impl IntoElement + use<> {
    let mut slot = div()
        .flex_1()
        .min_h_0()
        .w_full()
        .relative()
        .overflow_hidden();
    if let Some(err) = decode_error {
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
                .text_color(gpui::rgb(0xff5555))
                .child(err.clone()),
        );
    } else if let Some(tex) = image {
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
                .text_color(gpui::rgba(0x88888888))
                .child("Movie Player"),
        );
    }
    slot
}

fn transport_bar(
    viewer: &MovieViewer,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let is_playing = viewer.is_playing();
    let has_movie = viewer.frame_duration_us > 0 && viewer.decode_error.is_none();
    let pos = viewer.playback_position();
    let duration = viewer.total_duration;
    let progress_pct = if duration.as_secs_f64() > 0.0 {
        (pos.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32 * 100.0
    } else {
        0.0
    };
    let label = if is_playing { "⏸  Pause" } else { "▶  Play" };

    h_flex()
        .w_full()
        .px_3()
        .py_2()
        .gap_3()
        .items_center()
        .bg(theme.secondary)
        .child(
            Button::new("movie-play")
                .label(label)
                .small()
                .disabled(!has_movie)
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = movie_viewer_mut(this);
                    if viewer.playback.is_none() {
                        viewer.start_playback();
                    } else if viewer.is_paused() {
                        viewer.resume();
                    } else {
                        viewer.pause();
                    }
                    cx.notify();
                })),
        )
        .child(
            Button::new("movie-stop")
                .label("⏹  Stop")
                .small()
                .disabled(!has_movie)
                .on_click(cx.listener(|this, _, _, cx| {
                    movie_viewer_mut(this).stop_playback();
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex_1()
                .child(Progress::new().value(progress_pct).bg(theme.accent)),
        )
        .child(div().text_color(theme.muted_foreground).child(format!(
            "{} / {}",
            format_duration(pos),
            format_duration(duration)
        )))
}

fn info_bar(viewer: &MovieViewer, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let fps = if viewer.frame_duration_us > 0 {
        1_000_000.0 / viewer.frame_duration_us as f64
    } else {
        0.0
    };

    let mut row = h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(cell(viewer.source.name.clone()))
        .child(separator(theme.border));
    if let Some(fmt) = viewer.format {
        row = row
            .child(cell(fmt.to_string()))
            .child(separator(theme.border));
    }
    row.child(cell(format!("{}×{}", viewer.width, viewer.height)))
        .child(separator(theme.border))
        .child(cell(format!("{fps:.2} fps")))
        .child(separator(theme.border))
        .child(cell(format_duration(viewer.total_duration)))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}

/// Wrap an RGBA8 video frame into the BGRA `RenderImage` gpui expects.
/// Same R↔B swap pattern as the image / bam viewers.
fn frame_to_render_image(frame: MovieVideoFrame) -> Arc<RenderImage> {
    let mut buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(frame.width as u32, frame.height as u32, frame.pixels)
            .expect("movie frame pixel buffer length disagrees with declared dimensions");
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let f = Frame::new(buffer);
    Arc::new(RenderImage::new(SmallVec::from_elem(f, 1)))
}

fn movie_viewer_mut(app: &mut ExplorerApp) -> &mut MovieViewer {
    let trait_obj = &mut app
        .viewer
        .inner
        .as_mut()
        .expect("movie click fired without an active viewer")
        .viewer;
    (trait_obj.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<MovieViewer>()
        .expect("active viewer is not a MovieViewer")
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

// ─── Producer thread + shared state (ported verbatim from egui) ──────

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
