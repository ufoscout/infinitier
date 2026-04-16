use eframe::egui;
use infinitier_mve_decoder::{AudioChunk, MveDecoder, VideoFrame};
use rodio::{OutputStream, Sink, Source};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Audio source wrapping a Vec<i16>
// ---------------------------------------------------------------------------

struct PcmSource {
    samples: std::vec::IntoIter<i16>,
    channels: u16,
    sample_rate: u32,
}

impl Iterator for PcmSource {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        self.samples.next()
    }
}

impl Source for PcmSource {
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
    Frame(VideoFrame, Vec<AudioChunk>),
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

    // Audio
    _audio_stream: OutputStream,
    audio_sink: Sink,
}

impl MvePlayer {
    fn new(cc: &eframe::CreationContext<'_>, path: PathBuf) -> Self {
        let (tx, rx) = mpsc::sync_channel::<PlayerMsg>(4);

        // Decoder thread
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || decode_thread(path, tx, ctx));

        let (_stream, stream_handle) =
            OutputStream::try_default().expect("no audio output device");
        let sink = Sink::try_new(&stream_handle).expect("failed to create audio sink");

        MvePlayer {
            receiver: rx,
            current_texture: None,
            next_frame_at: Instant::now(),
            current_duration: Duration::from_millis(33),
            finished: false,
            frame_count: 0,
            _audio_stream: _stream,
            audio_sink: sink,
        }
    }
}

fn decode_thread(path: PathBuf, tx: SyncSender<PlayerMsg>, ctx: egui::Context) {
    let mut dec = match MveDecoder::open(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open {:?}: {e}", path);
            let _ = tx.send(PlayerMsg::Done);
            return;
        }
    };

    loop {
        match dec.next_frame() {
            Ok(Some(frame)) => {
                if tx.send(PlayerMsg::Frame(frame.video, frame.audio)).is_err() {
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
    let _ = tx.send(PlayerMsg::Done);
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
                Ok(PlayerMsg::Frame(video, audio)) => {
                    self.frame_count += 1;
                    self.current_duration =
                        Duration::from_micros(video.duration_us as u64).max(Duration::from_millis(1));
                    self.next_frame_at = now + self.current_duration;

                    // Upload video texture
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [video.width as usize, video.height as usize],
                        &video.pixels,
                    );
                    self.current_texture = Some(ctx.load_texture(
                        "mve_frame",
                        image,
                        egui::TextureOptions::NEAREST,
                    ));

                    // Queue audio
                    for chunk in audio {
                        self.audio_sink.append(PcmSource {
                            samples: chunk.samples.into_iter(),
                            channels: chunk.channels as u16,
                            sample_rate: chunk.sample_rate,
                        });
                    }
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
