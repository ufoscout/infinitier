use eframe::egui;
use infinitier_mve_decoder::{MveDecoder, VideoFrame};
use rodio::{OutputStream, Sink, Source};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Continuous streaming audio source
//
// Rather than appending many small PcmSources (one per video frame), we use
// a single source that lives for the whole file lifetime and reads from an
// mpsc channel.  This eliminates the per-chunk source transitions that rodio
// can only handle with a brief gap, which manifests as crackling.
// ---------------------------------------------------------------------------

struct StreamingAudioSource {
    rx: mpsc::Receiver<Vec<i16>>,
    current: std::vec::IntoIter<i16>,
    channels: u16,
    sample_rate: u32,
}

impl Iterator for StreamingAudioSource {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        loop {
            // Drain the current chunk first.
            if let Some(s) = self.current.next() {
                return Some(s);
            }
            // Current chunk exhausted — try to fetch the next one.
            match self.rx.try_recv() {
                Ok(chunk) => {
                    self.current = chunk.into_iter();
                    // Loop back to pull a sample from the new chunk.
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Decoder hasn't produced the next chunk yet.
                    // Return silence to avoid an underrun click.
                    return Some(0);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Sender was dropped — file is done.
                    return None;
                }
            }
        }
    }
}

impl Source for StreamingAudioSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

// ---------------------------------------------------------------------------
// Messages from decoder thread to UI thread
// ---------------------------------------------------------------------------

enum PlayerMsg {
    Frame(VideoFrame),
    Done,
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct MvePlayer {
    receiver: Receiver<PlayerMsg>,
    current_texture: Option<egui::TextureHandle>,
    next_frame_at: Instant,
    current_duration: Duration,
    finished: bool,
    frame_count: u32,

    // Keep OutputStream and Sink alive for the duration of playback.
    _audio_stream: OutputStream,
    _audio_sink: Sink,
}

impl MvePlayer {
    fn new(cc: &eframe::CreationContext<'_>, path: PathBuf) -> Self {
        let (video_tx, video_rx) = mpsc::sync_channel::<PlayerMsg>(8);
        // Unbounded audio channel so the decoder never blocks on audio.
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<i16>>();

        let (_stream, stream_handle) = OutputStream::try_default().expect("no audio output device");

        // The streaming source is created here; its sample format is
        // discovered from the first audio chunk.  We use a placeholder
        // (22050 Hz stereo) until the decoder sends actual format.
        // In practice rodio only reads channels/sample_rate once, so we
        // must know the format before appending.  We read the first chunk
        // synchronously via a one-shot channel.
        let (fmt_tx, fmt_rx) = mpsc::channel::<(u16, u32)>();

        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || decode_thread(path, video_tx, ctx, audio_tx, fmt_tx));

        // Wait for the decoder to tell us the audio format before we
        // create the streaming source.
        let (channels, sample_rate) = fmt_rx.recv().unwrap_or((2, 22050));

        let streaming_source = StreamingAudioSource {
            rx: audio_rx,
            current: Vec::new().into_iter(),
            channels,
            sample_rate,
        };

        let sink = Sink::try_new(&stream_handle).expect("failed to create audio sink");
        sink.append(streaming_source);

        MvePlayer {
            receiver: video_rx,
            current_texture: None,
            next_frame_at: Instant::now(),
            current_duration: Duration::from_millis(33),
            finished: false,
            frame_count: 0,
            _audio_stream: _stream,
            _audio_sink: sink,
        }
    }
}

/// Decode all audio from the file in a single fast pass.
/// Returns (channels, sample_rate, all_chunks).
fn pre_buffer_audio(path: &std::path::Path) -> (u16, u32, Vec<Vec<i16>>) {
    let mut dec = match MveDecoder::open(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("pre_buffer_audio: failed to open: {e}");
            return (2, 22050, Vec::new());
        }
    };

    let mut chunks: Vec<Vec<i16>> = Vec::new();
    let mut channels = 2u16;
    let mut sample_rate = 22050u32;

    loop {
        match dec.next_frame() {
            Ok(Some(frame)) => {
                for chunk in frame.audio {
                    if chunks.is_empty() {
                        channels = chunk.channels as u16;
                        sample_rate = chunk.sample_rate;
                    }
                    chunks.push(chunk.samples);
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("pre_buffer_audio decode error: {e}");
                break;
            }
        }
    }

    (channels, sample_rate, chunks)
}

fn decode_thread(
    path: PathBuf,
    video_tx: SyncSender<PlayerMsg>,
    ctx: egui::Context,
    audio_tx: mpsc::Sender<Vec<i16>>,
    fmt_tx: mpsc::Sender<(u16, u32)>,
) {
    // Phase 1 — pre-buffer ALL audio.
    //
    // The audio chunk duration (≈66.8 ms at 22050 Hz) is shorter than the
    // video frame period (≈70.8 ms at 14 fps).  If audio is only produced
    // one chunk at a time as video frames are decoded, the audio consumer
    // drains the buffer faster than the decoder refills it, causing underruns
    // (silence injections = crackling) after ~9 seconds.
    //
    // By pre-loading all audio upfront the streaming source always has the
    // full file in its queue, eliminating underruns entirely.
    let (channels, sample_rate, audio_chunks) = pre_buffer_audio(&path);
    let _ = fmt_tx.send((channels, sample_rate));
    for samples in audio_chunks {
        let _ = audio_tx.send(samples);
    }
    // Drop audio_tx now so the StreamingAudioSource ends when all samples
    // from the pre-buffer have been played.
    drop(audio_tx);

    // Phase 2 — decode and send video frames (paced by the sync channel).
    let mut dec = match MveDecoder::open(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open {:?}: {e}", path);
            let _ = video_tx.send(PlayerMsg::Done);
            return;
        }
    };

    loop {
        match dec.next_frame() {
            Ok(Some(frame)) => {
                if video_tx.send(PlayerMsg::Frame(frame.video)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("Decode error: {e}");
                break;
            }
        }
    }

    let _ = video_tx.send(PlayerMsg::Done);
    ctx.request_repaint();
}

impl eframe::App for MvePlayer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Consume all ready frames until the one we should show now.
        while !self.finished {
            let now = Instant::now();
            if now < self.next_frame_at {
                break;
            }

            match self.receiver.try_recv() {
                Ok(PlayerMsg::Frame(video)) => {
                    self.frame_count += 1;
                    self.current_duration = Duration::from_micros(video.duration_us as u64)
                        .max(Duration::from_millis(1));
                    self.next_frame_at = now + self.current_duration;

                    // Upload video texture
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [video.width as usize, video.height as usize],
                        &video.pixels,
                    );
                    self.current_texture =
                        Some(ctx.load_texture("mve_frame", image, egui::TextureOptions::NEAREST));
                }
                Ok(PlayerMsg::Done) => {
                    self.finished = true;
                    break;
                }
                Err(_) => break, // no frame ready yet
            }
        }

        // Schedule repaint for the next frame
        if !self.finished {
            let until_next = self.next_frame_at.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(until_next.max(Duration::from_millis(1)));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tex) = &self.current_texture {
                let available = ui.available_size();
                let img_size = tex.size_vec2();
                // Scale to fit, preserving aspect ratio
                let scale = (available.x / img_size.x).min(available.y / img_size.y);
                let display_size = img_size * scale;
                let offset = (available - display_size) * 0.5;
                ui.add_space(offset.y.max(0.0));
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.image((tex.id(), display_size));
                });
            } else if self.finished {
                ui.centered_and_justified(|ui| {
                    ui.heading("Playback finished");
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("Loading…");
                });
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("Usage: mve_player <file.mve>");
            std::process::exit(1);
        });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MVE Player")
            .with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MVE Player",
        options,
        Box::new(move |cc| Ok(Box::new(MvePlayer::new(cc, path.clone())))),
    )
}
