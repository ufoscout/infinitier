use infinitier_datasource::Reader;
use log::{debug, warn};
use std::{
    io::{BufRead, Seek, SeekFrom},
    path::Path,
};

use crate::{
    AudioChunk, MveFrame, VideoFormat, VideoFrame,
    audio::decompress_audio,
    error::Error,
    video::{decode_frame8, decode_frame16},
};

// ---------------------------------------------------------------------------
// MVE segment opcodes
// ---------------------------------------------------------------------------
const OC_END_OF_STREAM: u8 = 0x00;
const OC_END_OF_CHUNK: u8 = 0x01;
const OC_CREATE_TIMER: u8 = 0x02;
const OC_AUDIO_BUFFERS: u8 = 0x03;
const OC_PLAY_AUDIO: u8 = 0x04;
const OC_VIDEO_BUFFERS: u8 = 0x05;
const OC_PLAY_VIDEO: u8 = 0x07;
const OC_AUDIO_DATA: u8 = 0x08;
const OC_AUDIO_SILENCE: u8 = 0x09;
const OC_VIDEO_MODE: u8 = 0x0a;
const OC_PALETTE: u8 = 0x0c;
const OC_PALETTE_COMPRESSED: u8 = 0x0d;
const OC_CODE_MAP: u8 = 0x0f;
const OC_VIDEO_DATA: u8 = 0x11;

const AUDIO_FLAG_STEREO: u16 = 0x0001;
const AUDIO_FLAG_16BIT: u16 = 0x0002;
const AUDIO_FLAG_COMPRESSED: u16 = 0x0004;

const VIDEO_FLAG_DELTA: u16 = 0x0001;

const DEFAULT_AUDIO_STREAM: u16 = 0x0001;

// The first 24 bytes are fixed; the last 2 bytes vary between encoder versions.
const MVE_SIGNATURE_PREFIX: &[u8] = b"Interplay MVE File\x1a\x00\x1a\x00\x00\x01";

// ---------------------------------------------------------------------------
// Internal result from processing a segment/chunk
// ---------------------------------------------------------------------------
#[derive(PartialEq)]
enum StepResult {
    Ok,
    EndOfFrame,
    EndOfStream,
}

// ---------------------------------------------------------------------------
// Audio info retained across segments
// ---------------------------------------------------------------------------
#[derive(Default, Clone)]
struct AudioInfo {
    channels: u8,
    sample_rate: u32,
    bits: u8,
    compressed: bool,
}

// ---------------------------------------------------------------------------
// MveDecoder
// ---------------------------------------------------------------------------
/// Per-block coding-mode counters, accumulated as the decoder walks
/// every video frame. Useful for reverse-engineering an encoder's
/// behaviour: which of the 16 8×8 block opcodes does it emit, and how
/// often, on what kind of input?
///
/// Indexed by opcode (0..=15). Opcode meanings (8-bit Palette8):
/// `0x0`/`0x1` skip, `0x2` current-frame motion (low range),
/// `0x3` current-frame motion (full range), `0x4`/`0x5` previous-frame
/// motion, `0x7` "delta" pattern, `0x8`–`0xb` quad-of-quads modes,
/// `0xc`/`0xd` 4×4 / 8×4 colour fills, `0xe` solid colour,
/// `0xf` raw pixel block.  Opcode `0x6` is reserved/unused.
#[derive(Debug, Default, Clone)]
pub struct BlockModeStats {
    /// Opcode counts for the 8-bit (paletted) path.
    pub video8: [u64; 16],
    /// Opcode counts for the 16-bit (RGB555) path.
    pub video16: [u64; 16],
    /// Number of video frames seen.
    pub frames: u64,
    /// Total blocks counted across all frames.
    pub blocks: u64,
}

pub struct MveDecoder<R: BufRead + Seek> {
    reader: Reader<R>,

    /// Caller-supplied label (resource name, file path, …) prefixed to log
    /// records so consumers decoding many streams can tell entries apart.
    name: String,

    // Video state
    width: u16,
    height: u16,
    format: VideoFormat,
    palette: Box<[[u8; 4]; 256]>,
    /// Front frame buffer. `Box<[u8]>` (not `Vec`) because the size is
    /// fixed by `read_video_buffers` once and never grows after.
    buf1: Box<[u8]>,
    /// Back frame buffer (held across frames for delta decoding).
    buf2: Box<[u8]>,
    /// Per-frame code map. Resize-in-place via `Vec::resize` keeps the
    /// allocation warm across frames; `Vec` is the right type because
    /// `resize` is the cheap path here, not `push`.
    code_map: Vec<u8>,
    /// Scratch buffer for `OC_VIDEO_DATA` / `OC_AUDIO_DATA` payloads.
    /// Reused across frames so we don't allocate a fresh `Vec<u8>` per
    /// frame for the (often 10-50 KB) compressed video block stream
    /// and per-frame audio segments.
    scratch: Vec<u8>,

    // Audio state
    audio: AudioInfo,

    // Frame timing (microseconds per frame)
    frame_duration_us: u32,

    // Accumulated audio for the next video frame
    pending_audio: Vec<AudioChunk>,

    // Per-block mode statistics — written by the inner video decoders
    // every frame, exposed via `block_mode_stats()`.
    block_mode_stats: BlockModeStats,
}

impl<R: BufRead + Seek> MveDecoder<R> {
    // /// Open an MVE file from disk.
    // pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
    //     let f = BufReader::new(File::open(path)?);
    //     let reader = Reader {
    //         data: Box::new(f) as Box<dyn DataTrait>,
    //         charset: WINDOWS_1252,
    //     };
    //     Self::new(reader)
    // }

    /// Create a decoder from an `infinitier_datasource::Reader`.
    /// Validates the signature and pre-processes the initialisation chunks.
    ///
    /// `name` is a caller-supplied label (resource id, file path, …) that
    /// gets prefixed to every log record this decoder emits.
    pub fn new(mut reader: Reader<R>, name: impl Into<String>) -> Result<Self, Error> {
        let name = name.into();
        let sig = reader.read_exact::<26>()?;
        if &sig[..24] != MVE_SIGNATURE_PREFIX {
            log::error!("[{name}] Invalid MVE signature");
            return Err(Error::InvalidSignature);
        }

        let mut dec = MveDecoder {
            reader,
            name,
            width: 0,
            height: 0,
            format: VideoFormat::Palette8,
            // Pre-set alpha=0xff for every entry; `read_palette`
            // writes RGB into [0..3] and `build_video_frame` copies
            // the whole 4-byte slot, so untouched palette entries
            // stay opaque (matches the prior behaviour where the
            // alpha byte was always written as 0xff).
            palette: Box::new([[0u8, 0, 0, 0xff]; 256]),
            buf1: Box::default(),
            buf2: Box::default(),
            code_map: Vec::new(),
            scratch: Vec::new(),
            audio: AudioInfo::default(),
            frame_duration_us: 0,
            pending_audio: Vec::new(),
            block_mode_stats: BlockModeStats::default(),
        };

        // First two chunks contain all initialisation (audio/video setup)
        dec.process_chunk()?;
        dec.process_chunk()?;

        debug!(
            "[{}] MVE decoder ready: {}x{}, {:?}, frame_duration={}µs",
            dec.name, dec.width, dec.height, dec.format, dec.frame_duration_us
        );
        Ok(dec)
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn height(&self) -> u16 {
        self.height
    }
    pub fn format(&self) -> VideoFormat {
        self.format
    }
    /// Per-block coding-mode counters accumulated as frames are
    /// decoded. Use this to map the encoder's behaviour: which
    /// opcodes a given input causes the encoder to emit, and how
    /// often.
    pub fn block_mode_stats(&self) -> &BlockModeStats {
        &self.block_mode_stats
    }

    pub fn frame_duration_us(&self) -> u32 {
        self.frame_duration_us
    }

    /// Decode all audio from the stream and write it to a WAV file at `dest`.
    ///
    /// The file is created (or truncated) at `dest`.  The method consumes the
    /// decoder; if you need to continue decoding video afterwards open a fresh
    /// `MveDecoder`.
    pub fn extract_audio_to_wav(mut self, dest: impl AsRef<Path>) -> Result<(), Error> {
        // Stream samples straight to the WAV writer instead of
        // collecting the whole stream in `Vec<i16>` first — keeps
        // peak memory at one frame's audio chunk regardless of clip
        // length.
        //
        // The first audio chunk decides the WAV header (channels +
        // sample rate); we open the writer lazily on that frame so a
        // video with no audio doesn't produce a header-only WAV.
        let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;

        while let Some(frame) = self.next_frame()? {
            for chunk in frame.audio {
                let w = match writer.as_mut() {
                    Some(w) => w,
                    None => {
                        let spec = hound::WavSpec {
                            channels: chunk.channels as u16,
                            sample_rate: chunk.sample_rate,
                            bits_per_sample: 16,
                            sample_format: hound::SampleFormat::Int,
                        };
                        writer = Some(hound::WavWriter::create(dest.as_ref(), spec)?);
                        writer.as_mut().unwrap()
                    }
                };
                for &s in &chunk.samples {
                    w.write_sample(s)?;
                }
            }
        }

        // No audio chunks at all — emit an empty stereo / 22050 Hz
        // WAV so the destination file always exists, matching the
        // previous behaviour.
        let writer = match writer {
            Some(w) => w,
            None => hound::WavWriter::create(
                dest.as_ref(),
                hound::WavSpec {
                    channels: 2,
                    sample_rate: 22050,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )?,
        };
        writer.finalize()?;

        Ok(())
    }

    /// Decode and return the next complete video frame together with any
    /// audio chunks accumulated since the previous frame.
    /// Returns `None` when the stream ends.
    pub fn next_frame(&mut self) -> Result<Option<MveFrame>, Error> {
        loop {
            match self.process_chunk()? {
                StepResult::EndOfStream => return Ok(None),
                StepResult::EndOfFrame => {
                    let video = self.build_video_frame();
                    let audio = std::mem::take(&mut self.pending_audio);
                    return Ok(Some(MveFrame { video, audio }));
                }
                StepResult::Ok => {}
            }
        }
    }

    // ------------------------------------------------------------------
    // Internal: chunk / segment processing
    // ------------------------------------------------------------------

    fn read_u8(&mut self) -> Result<u8, Error> {
        Ok(self.reader.read_u8()?)
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        Ok(self.reader.read_u16()?)
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        Ok(self.reader.read_u32()?)
    }

    fn skip(&mut self, n: u64) -> Result<(), Error> {
        self.reader.data.seek(SeekFrom::Current(n as i64))?;
        Ok(())
    }

    /// Process one chunk (header + all its segments).
    /// Returns the last "interesting" result from the segments.
    fn process_chunk(&mut self) -> Result<StepResult, Error> {
        let chunk_size = self.read_u16()?;
        let _chunk_type = self.read_u16()?;
        let mut offset = 0u16;
        let mut last = StepResult::Ok;

        while offset < chunk_size {
            let seg_size = self.read_u16()?;
            let seg_type = self.read_u8()?;
            let seg_ver = self.read_u8()?;

            let result = self.process_segment(seg_size, seg_type, seg_ver)?;
            if result != StepResult::Ok {
                last = result;
            }
            offset = offset.saturating_add(4 + seg_size);
        }

        Ok(last)
    }

    fn process_segment(
        &mut self,
        size: u16,
        seg_type: u8,
        version: u8,
    ) -> Result<StepResult, Error> {
        match seg_type {
            OC_CREATE_TIMER => self.read_timer(),
            OC_AUDIO_BUFFERS => self.read_audio_buffers(version),
            OC_VIDEO_BUFFERS => self.read_video_buffers(version),
            OC_AUDIO_DATA => self.read_audio(false, size),
            OC_AUDIO_SILENCE => self.read_audio(true, size),
            OC_VIDEO_MODE => self.read_video_mode(),
            OC_PALETTE => self.read_palette(),
            OC_CODE_MAP => self.read_code_map(size),
            OC_VIDEO_DATA => self.read_video_data(size),
            OC_END_OF_STREAM => {
                self.skip(size as u64)?;
                return Ok(StepResult::EndOfStream);
            }
            OC_PLAY_VIDEO => {
                self.skip(size as u64)?;
                return Ok(StepResult::EndOfFrame);
            }
            OC_END_OF_CHUNK | OC_PLAY_AUDIO | OC_PALETTE_COMPRESSED | 0x13 | 0x14 | 0x15 => {
                self.skip(size as u64)?;
                return Ok(StepResult::Ok);
            }
            _ => {
                warn!(
                    "[{}] Unknown MVE segment type {:#04x}, skipping {} bytes",
                    self.name, seg_type, size
                );
                self.skip(size as u64)?;
                return Ok(StepResult::Ok);
            }
        }?;

        Ok(StepResult::Ok)
    }

    fn read_timer(&mut self) -> Result<(), Error> {
        let rate = self.read_u32()?;
        let subdiv = self.read_u16()?;
        self.frame_duration_us = rate.saturating_mul(subdiv as u32);
        Ok(())
    }

    fn read_audio_buffers(&mut self, version: u8) -> Result<(), Error> {
        let _unk = self.read_u16()?;
        let flags = self.read_u16()?;
        let sample_rate = self.read_u16()? as u32;
        let min_buf = self.read_u32()?;
        let _ = min_buf;

        self.audio.channels = if flags & AUDIO_FLAG_STEREO != 0 { 2 } else { 1 };
        self.audio.bits = if flags & AUDIO_FLAG_16BIT != 0 { 16 } else { 8 };
        self.audio.sample_rate = sample_rate;
        self.audio.compressed = version > 0 && (flags & AUDIO_FLAG_COMPRESSED != 0);
        Ok(())
    }

    fn read_video_buffers(&mut self, version: u8) -> Result<(), Error> {
        // Always 8 bytes: [w_blocks u16][h_blocks u16][buf_count u16][format u16]
        let w_blocks = self.read_u16()?; // bytes 0-1
        let h_blocks = self.read_u16()?; // bytes 2-3
        let _buf_count = self.read_u16()?; // bytes 4-5  (number of back buffers, unused)
        let format_flag = self.read_u16()?; // bytes 6-7 (format: 0=8bpp, non-zero=16bpp)

        self.width = w_blocks << 3;
        self.height = h_blocks << 3;
        // format_flag is only valid when version > 1
        self.format = if version > 1 && format_flag > 0 {
            VideoFormat::Rgb555
        } else {
            VideoFormat::Palette8
        };

        let bpp: usize = if self.format == VideoFormat::Rgb555 {
            2
        } else {
            1
        };
        let frame_bytes = self.width as usize * self.height as usize * bpp;
        self.buf1 = vec![0u8; frame_bytes].into_boxed_slice();
        self.buf2 = vec![0u8; frame_bytes].into_boxed_slice();
        Ok(())
    }

    fn read_video_mode(&mut self) -> Result<(), Error> {
        let _w = self.read_u16()?;
        let _h = self.read_u16()?;
        let _flags = self.read_u16()?;
        Ok(())
    }

    fn read_palette(&mut self) -> Result<(), Error> {
        let start = self.read_u16()? as usize;
        let count = self.read_u16()? as usize;
        for i in start..start + count {
            let r = self.read_u8()?;
            let g = self.read_u8()?;
            let b = self.read_u8()?;
            self.palette[i] = [r << 2, g << 2, b << 2, 0xff];
        }
        Ok(())
    }

    fn read_code_map(&mut self, size: u16) -> Result<(), Error> {
        // Resize-in-place keeps the `code_map` buffer warm across frames
        // — the underlying allocation is reused, so no alloc/free pair
        // per frame.
        let n = size as usize;
        self.code_map.resize(n, 0);
        self.reader.data.read_exact(&mut self.code_map)?;
        Ok(())
    }

    fn read_audio(&mut self, silence: bool, size: u16) -> Result<(), Error> {
        // 6-byte header: [seq u16][stream_mask u16][audio_size u16]
        let _seq = self.read_u16()?;
        let stream_mask = self.read_u16()?;
        let audio_size = self.read_u16()?;

        let data_size = size.saturating_sub(6) as usize;

        if stream_mask & DEFAULT_AUDIO_STREAM == 0 {
            self.skip(data_size as u64)?;
            return Ok(());
        }

        if silence {
            self.skip(data_size as u64)?;
            let samples = vec![0i16; audio_size as usize / 2];
            self.pending_audio.push(AudioChunk {
                channels: self.audio.channels,
                sample_rate: self.audio.sample_rate,
                samples,
            });
            return Ok(());
        }

        // Reuse the scratch buffer so we don't allocate fresh per audio segment.
        self.scratch.resize(data_size, 0);
        self.reader.data.read_exact(&mut self.scratch)?;

        let samples = if self.audio.compressed {
            decompress_audio(&self.scratch, audio_size, self.audio.channels)
        } else {
            // Raw i16 LE samples
            self.scratch
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect()
        };

        self.pending_audio.push(AudioChunk {
            channels: self.audio.channels,
            sample_rate: self.audio.sample_rate,
            samples,
        });
        Ok(())
    }

    fn read_video_data(&mut self, size: u16) -> Result<(), Error> {
        // Skip 12 bytes of metadata, then read flags
        self.skip(12)?;
        let flags = self.read_u16()?;
        let data_size = (size as usize).saturating_sub(14);

        if flags & VIDEO_FLAG_DELTA != 0 {
            std::mem::swap(&mut self.buf1, &mut self.buf2);
        }

        // Reuse the scratch buffer for the (often 10-50 KB) compressed
        // block stream — saves a fresh `Vec<u8>` allocation per frame.
        self.scratch.resize(data_size, 0);
        self.reader.data.read_exact(&mut self.scratch)?;

        if self.format == VideoFormat::Rgb555 {
            decode_frame16(
                &mut self.buf1,
                &mut self.buf2,
                &self.code_map,
                &self.scratch,
                self.width,
                self.height,
                &mut self.block_mode_stats,
            )?;
        } else {
            decode_frame8(
                &mut self.buf1,
                &mut self.buf2,
                &self.code_map,
                &self.scratch,
                self.width,
                self.height,
                &mut self.block_mode_stats,
            )?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Convert raw back-buffer pixels to RGBA
    // ------------------------------------------------------------------

    fn build_video_frame(&self) -> VideoFrame {
        let w = self.width as usize;
        let h = self.height as usize;
        let n_pixels = w * h;

        // Pre-fill the buffer to its exact size and write via slice
        // chunks instead of `Vec::push` per channel — saves four
        // capacity-checks and four bounds-checks per pixel.
        let mut rgba = vec![0u8; n_pixels * 4];
        match self.format {
            VideoFormat::Palette8 => {
                for (dst, &idx) in rgba.chunks_exact_mut(4).zip(self.buf1.iter()) {
                    dst.copy_from_slice(&self.palette[idx as usize]);
                }
            }
            VideoFormat::Rgb555 => {
                for (dst, src) in rgba.chunks_exact_mut(4).zip(self.buf1.chunks_exact(2)) {
                    let px = u16::from_le_bytes([src[0], src[1]]);
                    let r = ((px >> 10) & 0x1f) as u8;
                    let g = ((px >> 5) & 0x1f) as u8;
                    let b = (px & 0x1f) as u8;
                    dst[0] = (r << 3) | (r >> 2);
                    dst[1] = (g << 3) | (g >> 2);
                    dst[2] = (b << 3) | (b >> 2);
                    dst[3] = 0xff;
                }
            }
        }

        VideoFrame {
            width: self.width,
            height: self.height,
            pixels: rgba,
            duration_us: self.frame_duration_us,
        }
    }
}
