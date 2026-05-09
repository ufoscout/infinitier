//! High-level streaming decoder over `matroska-demuxer` +
//! `oxideav-vp8` + `lewton`.
//!
//! Mirrors the shape of [`infinitier_bik_decoder::BikStreamingDecoder`]
//! — `new(reader, name)` once, `next_frame()` until you get
//! `Ok(None)`. The pixel format is configurable: [`WbmOutputFormat::Yuv`]
//! (default — zero-cost, returns the codec's native YUV420p planes) or
//! [`WbmOutputFormat::Rgba`] (one BT.601 conversion per frame, returns
//! tightly-packed RGBA8 ready to feed straight into a GPU texture).

use std::io::{Read, Seek};

use matroska_demuxer::{Frame as MkvFrame, MatroskaFile, TrackType};
use oxideav_core::{CodecId, Frame as OxFrame, Packet, TimeBase};
use oxideav_core::registry::codec::Decoder as OxDecoder;
use oxideav_vp8::Vp8Decoder;

use crate::error::{WbmError, WbmResult};
use crate::vorbis::VorbisDriver;
use crate::yuv::yuv420p_to_rgba8;

/// Pixel format of the [`WbmVideoFrame`] returned by
/// [`WbmStreamingDecoder::next_frame`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WbmOutputFormat {
    /// Native 4:2:0 YUV planar output, no per-frame conversion.
    /// `chroma_*` resolution is half of `width / height` in both axes;
    /// chroma subsampling must be handled by the consumer (typically
    /// inside a GPU shader).
    #[default]
    Yuv,
    /// Tightly-packed RGBA8 (`width * height * 4` bytes), nearest-
    /// neighbour chroma upsample with BT.601 coefficients. One full
    /// frame copy per `next_frame` call; preferred when the consumer
    /// just wants RGBA pixels.
    Rgba,
}

/// Y/U/V planes of one decoded frame, with tight strides
/// (no MB padding). Lengths satisfy:
/// * `y.len()  == width * height`
/// * `u.len()  == ((width + 1) / 2) * ((height + 1) / 2)`
/// * `v.len()` matches `u.len()`
#[derive(Debug, Clone)]
pub struct WbmYuvFrame {
    pub width: u32,
    pub height: u32,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
    pub y_stride: u32,
    pub uv_stride: u32,
}

/// Pixel payload variant inside a [`WbmVideoFrame`]. Which arm is
/// populated is determined at decoder construction by
/// [`WbmOutputFormat`].
#[derive(Debug, Clone)]
pub enum WbmPixels {
    Yuv(WbmYuvFrame),
    /// Tightly-packed RGBA8 row-major, `width * height * 4` bytes.
    Rgba(Vec<u8>),
}

/// One decoded video frame plus its presentation duration.
#[derive(Debug, Clone)]
pub struct WbmVideoFrame {
    pub width: u32,
    pub height: u32,
    /// How long to display this frame, in microseconds. Stable across
    /// the stream — WebM track `default_duration` (nanoseconds)
    /// truncated to microseconds.
    pub duration_us: u32,
    pub pixels: WbmPixels,
}

/// PCM audio data accompanying a video frame (interleaved s16 LE
/// samples, one or two channels).
#[derive(Debug, Clone)]
pub struct WbmAudioChunk {
    pub channels: u8,
    pub sample_rate: u32,
    /// Interleaved signed 16-bit PCM samples.
    pub samples: Vec<i16>,
}

/// A complete decoded frame: one video frame plus any audio chunks
/// that arrived before it in the container.
#[derive(Debug, Clone)]
pub struct WbmFrame {
    pub video: WbmVideoFrame,
    pub audio: Vec<WbmAudioChunk>,
}

/// Streaming WBM decoder. Owns the Matroska demuxer, the VP8 decoder
/// and the Vorbis driver, and pulls one [`WbmFrame`] per
/// [`Self::next_frame`] call until the input is exhausted.
pub struct WbmStreamingDecoder<R: Read + Seek> {
    mkv: MatroskaFile<R>,
    name: String,
    output_format: WbmOutputFormat,

    video_track: u64,
    width: u32,
    height: u32,
    frame_duration_us: u32,
    total_duration_us: u64,

    audio: Option<AudioState>,

    vp8: Vp8Decoder,
    /// Audio chunks that arrived between the previous returned video
    /// frame and the next one. Drained when `next_frame` produces a
    /// visible VP8 frame.
    pending_audio: Vec<WbmAudioChunk>,
    /// Reusable demuxer scratch frame.
    scratch: MkvFrame,
}

struct AudioState {
    track: u64,
    driver: VorbisDriver,
}

impl<R: Read + Seek> WbmStreamingDecoder<R> {
    /// Build a decoder positioned at byte 0 of `reader`. Defaults to
    /// [`WbmOutputFormat::Yuv`]; call [`Self::with_output_format`]
    /// for RGBA.
    pub fn new(reader: R, name: impl Into<String>) -> WbmResult<Self> {
        let mkv = MatroskaFile::open(reader)?;

        let mut video_track: Option<u64> = None;
        let mut video_w = 0u64;
        let mut video_h = 0u64;
        let mut frame_duration_ns: u64 = 0;
        let mut audio: Option<AudioState> = None;

        for t in mkv.tracks() {
            match t.track_type() {
                TrackType::Video if video_track.is_none() && t.codec_id() == "V_VP8" => {
                    video_track = Some(t.track_number().get());
                    if let Some(v) = t.video() {
                        video_w = v.pixel_width().get();
                        video_h = v.pixel_height().get();
                    }
                    if let Some(d) = t.default_duration() {
                        frame_duration_ns = d.get();
                    }
                }
                TrackType::Audio if audio.is_none() && t.codec_id() == "A_VORBIS" => {
                    if let Some(priv_) = t.codec_private() {
                        let driver = VorbisDriver::from_codec_private(priv_)?;
                        audio = Some(AudioState {
                            track: t.track_number().get(),
                            driver,
                        });
                    }
                }
                _ => {}
            }
        }

        let video_track = video_track.ok_or(WbmError::NoVideoTrack)?;
        if video_w == 0 || video_h == 0 {
            return Err(WbmError::MissingDimensions);
        }
        // `default_duration` is in nanoseconds. Truncate to microseconds.
        let frame_duration_us = (frame_duration_ns / 1_000) as u32;

        // Total duration: `Info::duration` is in `timestamp_scale`
        // ticks (typically `1_000_000` ns = 1 ms). Both fields
        // optional in the spec; fall back to 0 when absent and let
        // callers derive duration from `frame_count` if they care.
        let info = mkv.info();
        let timestamp_scale_ns = info.timestamp_scale().get();
        let total_duration_us = info
            .duration()
            .map(|d| (d * (timestamp_scale_ns as f64) / 1_000.0) as u64)
            .unwrap_or(0);

        let vp8 = Vp8Decoder::new(CodecId("vp8".to_string()));

        log::debug!(
            "[wbm] {} ready: {}x{}, frame_dur={}us, total={}us, audio={}",
            "name-placeholder",
            video_w,
            video_h,
            frame_duration_us,
            total_duration_us,
            audio.is_some(),
        );

        Ok(Self {
            mkv,
            name: name.into(),
            output_format: WbmOutputFormat::default(),
            video_track,
            width: video_w as u32,
            height: video_h as u32,
            frame_duration_us,
            total_duration_us,
            audio,
            vp8,
            pending_audio: Vec::new(),
            scratch: MkvFrame::default(),
        })
    }

    /// Builder-style override for the output pixel format.
    pub fn with_output_format(mut self, format: WbmOutputFormat) -> Self {
        self.output_format = format;
        self
    }

    /// Caller-supplied label (resource id, file path, …).
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn output_format(&self) -> WbmOutputFormat {
        self.output_format
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Constant per-frame display duration in microseconds. Read from
    /// the Matroska `DefaultDuration` element of the video track; `0`
    /// if the source omits it.
    pub fn frame_duration_us(&self) -> u32 {
        self.frame_duration_us
    }

    /// Total duration of the stream in microseconds, or `0` if the
    /// container's `Info::Duration` element is missing. Computed once
    /// at construction.
    pub fn total_duration_us(&self) -> u64 {
        self.total_duration_us
    }

    /// Whether the source file carries a Vorbis audio track.
    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }

    /// Pull the next [`WbmFrame`] (one visible VP8 frame plus the
    /// Vorbis chunks that preceded it in the container). Returns
    /// `Ok(None)` once the demuxer reaches end-of-stream.
    pub fn next_frame(&mut self) -> WbmResult<Option<WbmFrame>> {
        loop {
            // Demux one block. matroska-demuxer reuses the scratch
            // `Frame`'s internal `Vec<u8>` across calls.
            if !self.mkv.next_frame(&mut self.scratch)? {
                // EOS — drop any orphaned audio that arrived after the
                // last video frame. Spec allows it but there's nothing
                // useful to attach it to.
                self.pending_audio.clear();
                return Ok(None);
            }

            if self.scratch.track == self.video_track {
                if let Some(frame) = self.send_video_packet()? {
                    return Ok(Some(frame));
                }
                continue;
            }

            if let Some(state) = self.audio.as_mut()
                && self.scratch.track == state.track
            {
                let pcm = state.driver.decode(&self.scratch.data)?;
                push_audio(&mut self.pending_audio, pcm, state.driver.channels, state.driver.sample_rate);
            }
            // Tracks we don't care about (subs, chapters, …) are
            // implicitly skipped by falling through.
        }
    }

    /// Feed one VP8 packet to the decoder. Returns the next visible
    /// frame if one is now available, or `None` when the packet was a
    /// hidden alt-ref frame (decoder updated the reference slot but
    /// emits nothing) and the caller should pull the next packet.
    fn send_video_packet(&mut self) -> WbmResult<Option<WbmFrame>> {
        // Move the demuxer's payload into a fresh `Packet` — the
        // demuxer reuses its `Frame`'s buffer between calls.
        let payload = std::mem::take(&mut self.scratch.data);
        let pkt = Packet {
            stream_index: 0,
            time_base: TimeBase::new(1, 1_000_000),
            pts: Some(self.scratch.timestamp as i64),
            dts: Some(self.scratch.timestamp as i64),
            duration: self.scratch.duration.map(|d| d as i64),
            flags: Default::default(),
            data: payload,
        };
        self.vp8.send_packet(&pkt)?;

        match self.vp8.receive_frame() {
            Ok(OxFrame::Video(vf)) => {
                let pixels = self.frame_to_pixels(&vf);
                let audio = std::mem::take(&mut self.pending_audio);
                Ok(Some(WbmFrame {
                    video: WbmVideoFrame {
                        width: self.width,
                        height: self.height,
                        duration_us: self.frame_duration_us,
                        pixels,
                    },
                    audio,
                }))
            }
            Ok(_) => {
                // VP8 never emits non-Video, but the enum is
                // `non_exhaustive` so we have to cover it.
                Ok(None)
            }
            Err(oxideav_core::Error::NeedMore) => Ok(None),
            Err(e) => Err(WbmError::Vp8(e)),
        }
    }

    fn frame_to_pixels(&self, vf: &oxideav_core::VideoFrame) -> WbmPixels {
        let y = &vf.planes[0];
        let u = &vf.planes[1];
        let v = &vf.planes[2];
        match self.output_format {
            WbmOutputFormat::Yuv => WbmPixels::Yuv(WbmYuvFrame {
                width: self.width,
                height: self.height,
                y: y.data.clone(),
                u: u.data.clone(),
                v: v.data.clone(),
                y_stride: y.stride as u32,
                uv_stride: u.stride as u32,
            }),
            WbmOutputFormat::Rgba => WbmPixels::Rgba(yuv420p_to_rgba8(
                &y.data,
                &u.data,
                &v.data,
                y.stride,
                u.stride,
                self.width,
                self.height,
            )),
        }
    }
}

fn push_audio(
    out: &mut Vec<WbmAudioChunk>,
    per_channel: Vec<Vec<i16>>,
    channels: u8,
    sample_rate: u32,
) {
    if per_channel.is_empty() {
        return;
    }
    let nch = per_channel.len();
    let n = per_channel[0].len();
    if n == 0 {
        return;
    }
    let mut interleaved = Vec::with_capacity(n * nch);
    for i in 0..n {
        for ch in &per_channel {
            interleaved.push(ch[i]);
        }
    }
    out.push(WbmAudioChunk {
        channels,
        sample_rate,
        samples: interleaved,
    });
}
