use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    audio::decompress_audio,
    error::Error,
    video::{decode_frame16, decode_frame8},
    AudioChunk, MveFrame, VideoFormat, VideoFrame,
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
pub struct MveDecoder<R: Read + Seek> {
    reader: R,

    // Video state
    width: u16,
    height: u16,
    format: VideoFormat,
    palette: Box<[[u8; 4]; 256]>,
    buf1: Vec<u8>,
    buf2: Vec<u8>,
    code_map: Vec<u8>,

    // Audio state
    audio: AudioInfo,

    // Frame timing (microseconds per frame)
    frame_duration_us: u32,

    // Accumulated audio for the next video frame
    pending_audio: Vec<AudioChunk>,
}

impl MveDecoder<BufReader<File>> {
    /// Open an MVE file from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let f = BufReader::new(File::open(path)?);
        Self::new(f)
    }
}

impl<R: Read + Seek> MveDecoder<R> {
    /// Create a decoder from any `Read + Seek` source.
    /// Validates the signature and pre-processes the initialisation chunks.
    pub fn new(mut reader: R) -> Result<Self, Error> {
        // Read and verify signature (26 bytes; last 2 bytes vary by encoder version)
        let mut sig = [0u8; 26];
        reader.read_exact(&mut sig)?;
        if &sig[..24] != MVE_SIGNATURE_PREFIX {
            return Err(Error::InvalidSignature);
        }

        let mut dec = MveDecoder {
            reader,
            width: 0,
            height: 0,
            format: VideoFormat::Palette8,
            palette: Box::new([[0u8; 4]; 256]),
            buf1: Vec::new(),
            buf2: Vec::new(),
            code_map: Vec::new(),
            audio: AudioInfo::default(),
            frame_duration_us: 0,
            pending_audio: Vec::new(),
        };

        // First two chunks contain all initialisation (audio/video setup)
        dec.process_chunk()?;
        dec.process_chunk()?;

        Ok(dec)
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    pub fn width(&self) -> u16 { self.width }
    pub fn height(&self) -> u16 { self.height }
    pub fn format(&self) -> VideoFormat { self.format }
    pub fn frame_duration_us(&self) -> u32 { self.frame_duration_us }

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
        let mut b = [0u8; 1];
        self.reader.read_exact(&mut b)?;
        Ok(b[0])
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        let mut b = [0u8; 2];
        self.reader.read_exact(&mut b)?;
        Ok(u16::from_le_bytes(b))
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        let mut b = [0u8; 4];
        self.reader.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_exact_vec(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let mut v = vec![0u8; n];
        self.reader.read_exact(&mut v)?;
        Ok(v)
    }

    fn skip(&mut self, n: u64) -> Result<(), Error> {
        self.reader.seek(SeekFrom::Current(n as i64))?;
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

    fn process_segment(&mut self, size: u16, seg_type: u8, version: u8) -> Result<StepResult, Error> {
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
            OC_END_OF_CHUNK
            | OC_PLAY_AUDIO
            | OC_PALETTE_COMPRESSED
            | 0x13 | 0x14 | 0x15 => {
                self.skip(size as u64)?;
                return Ok(StepResult::Ok);
            }
            _ => {
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
        let w_blocks = self.read_u16()?;   // bytes 0-1
        let h_blocks = self.read_u16()?;   // bytes 2-3
        let _buf_count = self.read_u16()?; // bytes 4-5  (number of back buffers, unused)
        let format_flag = self.read_u16()?; // bytes 6-7 (format: 0=8bpp, non-zero=16bpp)

        self.width = w_blocks << 3;
        self.height = h_blocks << 3;
        // format_flag is only valid when version > 1
        self.format = if version > 1 && format_flag > 0 { VideoFormat::Rgb555 } else { VideoFormat::Palette8 };

        let bpp: usize = if self.format == VideoFormat::Rgb555 { 2 } else { 1 };
        let frame_bytes = self.width as usize * self.height as usize * bpp;
        self.buf1 = vec![0u8; frame_bytes];
        self.buf2 = vec![0u8; frame_bytes];
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
        self.code_map = self.read_exact_vec(size as usize)?;
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

        let raw = self.read_exact_vec(data_size)?;

        let samples = if self.audio.compressed {
            decompress_audio(&raw, audio_size, self.audio.channels)
        } else {
            // Raw i16 LE samples
            raw.chunks_exact(2)
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

        let raw = self.read_exact_vec(data_size)?;

        if self.format == VideoFormat::Rgb555 {
            decode_frame16(&mut self.buf1, &mut self.buf2, &self.code_map, &raw, self.width, self.height)?;
        } else {
            decode_frame8(&mut self.buf1, &mut self.buf2, &self.code_map, &raw, self.width, self.height)?;
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
        let mut rgba = Vec::with_capacity(n_pixels * 4);

        match self.format {
            VideoFormat::Palette8 => {
                for &idx in &self.buf1 {
                    let c = self.palette[idx as usize];
                    rgba.push(c[0]);
                    rgba.push(c[1]);
                    rgba.push(c[2]);
                    rgba.push(0xff); // alpha is always opaque
                }
            }
            VideoFormat::Rgb555 => {
                for chunk in self.buf1.chunks_exact(2) {
                    let px = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let r = ((px >> 10) & 0x1f) as u8;
                    let g = ((px >> 5) & 0x1f) as u8;
                    let b = (px & 0x1f) as u8;
                    rgba.push((r << 3) | (r >> 2));
                    rgba.push((g << 3) | (g >> 2));
                    rgba.push((b << 3) | (b >> 2));
                    rgba.push(0xff);
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
