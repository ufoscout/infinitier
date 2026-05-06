use clap::Parser;
use eframe::egui;
use infinitier_datasource::DataSource;
use infinitier_mve_decoder::{MveDecoder, VideoFrame};
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::{
    num::NonZero,
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(author, version, about = "Play Interplay MVE video files")]
struct Args {
    /// Path to the MVE file to play.
    mve_file: PathBuf,
    /// Log filter, e.g. "warn", "debug", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,info")]
    log: String,
}

// ---------------------------------------------------------------------------
// Continuous streaming audio source
//
// Rather than appending many small PcmSources (one per video frame), we use
// a single source that lives for the whole file lifetime and reads from an
// mpsc channel.  This eliminates the per-chunk source transitions that rodio
// can only handle with a brief gap, which manifests as crackling.
// ---------------------------------------------------------------------------

struct StreamingAudioSource {
    rx: mpsc::Receiver<Vec<f32>>,
    current: std::vec::IntoIter<f32>,
    channels: NonZero<u16>,
    sample_rate: NonZero<u32>,
}

impl Iterator for StreamingAudioSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
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
                    return Some(0.0);
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
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> NonZero<u16> {
        self.channels
    }
    fn sample_rate(&self) -> NonZero<u32> {
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

    // Keep MixerDeviceSink and Player alive for the duration of playback.
    _audio_stream: MixerDeviceSink,
    _audio_sink: Player,
}

impl MvePlayer {
    fn new(cc: &eframe::CreationContext<'_>, path: PathBuf) -> Self {
        let (video_tx, video_rx) = mpsc::sync_channel::<PlayerMsg>(8);
        // Unbounded audio channel so the decoder never blocks on audio.
        let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>();

        let mut audio_device =
            DeviceSinkBuilder::open_default_sink().expect("no audio output device");
        audio_device.log_on_drop(false);

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
        let (channels_raw, sample_rate_raw) = fmt_rx.recv().unwrap_or((2, 22050));
        let channels = NonZero::new(channels_raw).unwrap_or(NonZero::new(2).unwrap());
        let sample_rate = NonZero::new(sample_rate_raw).unwrap_or(NonZero::new(22050).unwrap());

        let streaming_source = StreamingAudioSource {
            rx: audio_rx,
            current: Vec::new().into_iter(),
            channels,
            sample_rate,
        };

        let sink = Player::connect_new(audio_device.mixer());
        sink.append(streaming_source);

        MvePlayer {
            receiver: video_rx,
            current_texture: None,
            next_frame_at: Instant::now(),
            current_duration: Duration::from_millis(33),
            finished: false,
            frame_count: 0,
            _audio_stream: audio_device,
            _audio_sink: sink,
        }
    }
}

/// Decode all audio from the file in a single fast pass.
/// Returns (channels, sample_rate, all_chunks).
fn pre_buffer_audio(path: &std::path::Path) -> (u16, u32, Vec<Vec<f32>>) {
    let name = path.display().to_string();
    let data = DataSource::new(path);
    let reader = data.reader().unwrap();
    let mut dec = match MveDecoder::new(reader, name) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("pre_buffer_audio: failed to open: {e}");
            return (2, 22050, Vec::new());
        }
    };

    let mut chunks: Vec<Vec<f32>> = Vec::new();
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
                    let f32_samples: Vec<f32> =
                        chunk.samples.iter().map(|&s| s as f32 / 32768.0).collect();
                    chunks.push(f32_samples);
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
    audio_tx: mpsc::Sender<Vec<f32>>,
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
    let name = path.display().to_string();
    let data = DataSource::new(path.as_path());
    let reader = data.reader().unwrap();
    let mut dec = match MveDecoder::new(reader, name) {
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
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
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
    let args = Args::parse();
    env_logger::Builder::new().parse_filters(&args.log).init();
    let path = args.mve_file;

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
