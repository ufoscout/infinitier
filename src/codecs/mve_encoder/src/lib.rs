#![doc = include_str!("../readme.md")]

use std::io::{self, Write};

use log::debug;
use thiserror::Error as ThisError;

mod dpcm;
mod from_assets;
pub use from_assets::{encode_from_assets, FromAssetsError, FromAssetsOptions};

// ─── format constants ────────────────────────────────────────────────────────

/// Fixed 24-byte signature (matches `MVE_SIGNATURE_PREFIX` in the
/// decoder); bytes 24-25 are an arbitrary encoder-version pair that
/// the decoder ignores.
const SIGNATURE_24: &[u8] = b"Interplay MVE File\x1a\x00\x1a\x00\x00\x01";
/// Two padding bytes after the 24-byte prefix. Real avi2mve writes
/// `0x33 0x11` here; any value works.
const SIGNATURE_TAIL: [u8; 2] = [0x33, 0x11];

// Chunk type IDs — purely conventional; our decoder doesn't validate
// them. We pick the same numbers `avi2mve` writes for compatibility
// with stricter readers (gemrb's MVEPlayer checks them).
const CHUNK_INIT_VIDEO: u16 = 0x0002;
const CHUNK_INIT_AUDIO: u16 = 0x0000;
const CHUNK_FRAME: u16 = 0x0001;
const CHUNK_END: u16 = 0x0004;

// Segment opcodes (a.k.a. seg_type), kept in sync with the decoder.
const OC_END_OF_STREAM: u8 = 0x00;
const OC_END_OF_CHUNK: u8 = 0x01;
const OC_CREATE_TIMER: u8 = 0x02;
const OC_AUDIO_BUFFERS: u8 = 0x03;
const OC_VIDEO_BUFFERS: u8 = 0x05;
const OC_PLAY_VIDEO: u8 = 0x07;
const OC_AUDIO_DATA: u8 = 0x08;
const OC_VIDEO_MODE: u8 = 0x0a;
const OC_PALETTE: u8 = 0x0c;
const OC_CODE_MAP: u8 = 0x0f;
const OC_VIDEO_DATA: u8 = 0x11;

const VIDEO_FLAG_DELTA: u16 = 0x0001;
const AUDIO_FLAG_STEREO: u16 = 0x0001;
const AUDIO_FLAG_16BIT: u16 = 0x0002;
const AUDIO_FLAG_COMPRESSED: u16 = 0x0004;
const DEFAULT_AUDIO_STREAM: u16 = 0x0001;

/// Block coding opcodes used by the encoder. See the readme for what
/// each means and how the chooser picks between them.
const BLOCK_COPY_PREV: u8 = 0x0;
const BLOCK_MOTION_PREV: u8 = 0x4;
const BLOCK_DELTA: u8 = 0x7;
/// Mode `0x8` — "2-colour per partition": three sub-modes (4 quadrants
/// × 2 colours = 16 bytes; 2 vertical or horizontal halves × 2 colours
/// = 12 bytes). Branch selected by `p[0] <= p[1]` and `p[2] <= p[3]`
/// in the decoder.
const BLOCK_QUADRANT_PAIRS: u8 = 0x8;
const BLOCK_QUAD_PATTERN: u8 = 0x9;
/// Mode `0xa` — "4-colour per partition": three sub-modes (4 quadrants
/// × 4 colours = 32 bytes; 2 vertical or horizontal halves × 4 colours
/// = 24 bytes).
const BLOCK_QUADRANT_QUADS: u8 = 0xa;
const BLOCK_RAW: u8 = 0xb;
const BLOCK_4X4_FILL: u8 = 0xc;
const BLOCK_QUADRANTS: u8 = 0xd;
const BLOCK_SOLID: u8 = 0xe;

// ─── error type ──────────────────────────────────────────────────────────────

pub type Result<T> = std::result::Result<T, MveEncodeError>;

#[derive(Debug, ThisError)]
pub enum MveEncodeError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("dimensions {width}x{height} are invalid: must be non-zero, multiples of 8, ≤ 65535")]
    InvalidDimensions { width: u16, height: u16 },
    #[error("pixel buffer is the wrong size: got {got}, expected {expected}")]
    PixelBufferSize { got: usize, expected: usize },
    #[error("frame_duration_us must be ≥ 1, got {0}")]
    InvalidFrameDuration(u32),
    #[error("frames slice is empty — at least one frame is required")]
    NoFrames,
    #[error("a chunk's segment payload exceeds 65535 bytes")]
    ChunkTooBig,
    #[error("audio sample rate {0} exceeds the format's u16 limit of 65535 Hz")]
    AudioSampleRateTooHigh(u32),
    #[error("audio channels must be 1 (mono) or 2 (stereo), got {0}")]
    AudioChannelsInvalid(u16),
    #[error(
        "audio_samples_per_frame length {got} does not match video frame count {expected}"
    )]
    AudioFramesMismatch { got: usize, expected: usize },
}

// ─── public API ──────────────────────────────────────────────────────────────

/// 8-bit paletted still image to encode as a static MVE.
pub struct StaticImage {
    /// Width in pixels. Must be > 0 and a multiple of 8.
    pub width: u16,
    /// Height in pixels. Must be > 0 and a multiple of 8.
    pub height: u16,
    /// Palette indices, row-major, length `width * height`.
    pub pixels: Vec<u8>,
    /// 256 RGB triples, 8-bit per channel. The encoder quantises to
    /// 6-bit during write (MVE stores `value >> 2` per channel).
    pub palette: Box<[[u8; 3]; 256]>,
}

/// Audio configuration. Samples are i16 PCM on the wire and always
/// encoded as **Interplay DPCM** in the produced `.mve` (the format
/// the engine expects and what avi2mve / real game cutscenes ship).
/// DPCM is mildly lossy: reconstruction is bit-exact on silence and
/// stays within a few LSB on smooth content, but high-entropy
/// material (noise, abrupt transients) can drift further as the
/// predictor catches up.
#[derive(Clone, Copy)]
pub struct AudioOptions {
    /// Samples per second. Must be ≤ 65535 (format stores it as u16).
    pub sample_rate: u32,
    /// 1 for mono, 2 for stereo. Stereo samples are interleaved L,R,L,R,…
    pub channels: u16,
}

/// Multi-frame encode options shared between every frame.
pub struct VideoOptions {
    pub width: u16,
    pub height: u16,
    pub frame_duration_us: u32,
    pub palette: Box<[[u8; 3]; 256]>,
    /// When `true`, blocks that would otherwise be emitted as `0xb`
    /// raw 8×8 (64 bytes per block) instead use `0xc` 4×4 fill (16
    /// bytes), with a lossy 2×2 downsample that keeps the top-left
    /// pixel of each 2×2 sub-block.
    ///
    /// Necessary for high-detail content like random noise whose
    /// fully-raw frame would exceed MVE's 65,535-byte per-segment
    /// limit. Default `false` — the encoder is lossless for any
    /// content that fits.
    pub lossy_downsample: bool,
}

/// Encode a uniform-colour rectangle as `frame_count` frames of MVE.
/// First frame uses `0xe` for every block; subsequent frames use
/// `0x0` (skip with the `VIDEO_FLAG_DELTA` swap trick).
pub fn encode_solid_colour_video<W: Write>(
    width: u16,
    height: u16,
    rgb: [u8; 3],
    frame_count: u32,
    frame_duration_us: u32,
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    validate_dims(width, height)?;
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = rgb; // index 0 stays black so palette[0] != rgb if rgb != black
    let pixels = vec![1u8; width as usize * height as usize];
    let img = StaticImage {
        width,
        height,
        pixels,
        palette,
    };
    encode_static_palette8(&img, frame_count, frame_duration_us, name, out)
}

/// Encode an 8-bit paletted still image as `frame_count` frames of
/// static MVE — every frame shows the same image. Internally calls
/// [`encode_video`] with the image replicated `frame_count` times,
/// so it accepts any input where each 8×8 block is encodable by
/// Phase-2 modes (≤ 2 distinct colours, *or* 3–4 distinct colours
/// arranged as uniform 4×4 quadrants).
pub fn encode_static_palette8<W: Write>(
    image: &StaticImage,
    frame_count: u32,
    frame_duration_us: u32,
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    if image.pixels.len() != image.width as usize * image.height as usize {
        return Err(MveEncodeError::PixelBufferSize {
            got: image.pixels.len(),
            expected: image.width as usize * image.height as usize,
        });
    }
    let opts = VideoOptions {
        width: image.width,
        height: image.height,
        frame_duration_us,
        palette: image.palette.clone(),
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = std::iter::repeat(image.pixels.as_slice())
        .take(frame_count.max(1) as usize)
        .collect();
    encode_video(&opts, &frames, name, out)
}

/// Encode an arbitrary multi-frame video. Each entry in `frames` is a
/// `width * height`-byte palette-index buffer, row-major. Frames are
/// independently analysed; unchanged blocks emit `0x0` (skip), and
/// changed blocks emit the cheapest mode that fits.
pub fn encode_video<W: Write>(
    options: &VideoOptions,
    frames: &[&[u8]],
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    encode_av(options, frames, None, name, out)
}

/// Encode a multi-frame video with optional 16-bit-PCM audio.
///
/// `audio` is `Some((opts, samples_per_frame))` where
/// `samples_per_frame[i]` are the audio samples bundled into the
/// chunk for video frame `i`. For stereo, samples are interleaved
/// L, R, L, R… Length of `samples_per_frame` must equal `frames.len()`.
pub fn encode_av<W: Write>(
    options: &VideoOptions,
    frames: &[&[u8]],
    audio: Option<(&AudioOptions, &[Vec<i16>])>,
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    let name = name.into();
    validate_dims(options.width, options.height)?;
    if options.frame_duration_us == 0 {
        return Err(MveEncodeError::InvalidFrameDuration(options.frame_duration_us));
    }
    if frames.is_empty() {
        return Err(MveEncodeError::NoFrames);
    }
    let expected = options.width as usize * options.height as usize;
    for f in frames.iter() {
        if f.len() != expected {
            return Err(MveEncodeError::PixelBufferSize {
                got: f.len(),
                expected,
            });
        }
    }
    if let Some((aopts, per_frame)) = audio {
        validate_audio(aopts)?;
        if per_frame.len() != frames.len() {
            return Err(MveEncodeError::AudioFramesMismatch {
                got: per_frame.len(),
                expected: frames.len(),
            });
        }
    }

    let bw = (options.width as usize) >> 3;
    let bh = (options.height as usize) >> 3;
    let n_blocks = bw * bh;
    let stride = options.width as usize;

    // ── 1. signature ──────────────────────────────────────────────────────
    out.write_all(SIGNATURE_24)?;
    out.write_all(&SIGNATURE_TAIL)?;

    // ── 2. init video chunk ───────────────────────────────────────────────
    let init_video = build_init_video_chunk(
        options.width,
        options.height,
        options.frame_duration_us,
        options.palette.as_ref(),
    )?;
    write_chunk(out, CHUNK_INIT_VIDEO, &init_video)?;

    // ── 3. init audio chunk (empty if no audio) ──────────────────────────
    let init_audio = match audio {
        Some((aopts, _)) => build_init_audio_chunk(aopts)?,
        None => Vec::new(),
    };
    write_chunk(out, CHUNK_INIT_AUDIO, &init_audio)?;

    // ── 4. per-frame chunks ──────────────────────────────────────────────
    for (frame_idx, frame_pixels) in frames.iter().enumerate() {
        let prev = if frame_idx == 0 {
            None
        } else {
            Some(frames[frame_idx - 1])
        };
        let frame_audio = audio.map(|(aopts, per_frame)| (*aopts, per_frame[frame_idx].as_slice()));
        let body = build_frame_chunk(
            frame_pixels,
            prev,
            stride,
            bw,
            bh,
            n_blocks,
            frame_idx > 0,
            options.lossy_downsample,
            frame_audio,
            frame_idx as u16,
        )?;
        write_chunk(out, CHUNK_FRAME, &body)?;
    }

    // ── 5. end-of-stream chunk ────────────────────────────────────────────
    let mut end = Vec::with_capacity(4);
    write_segment(&mut end, OC_END_OF_STREAM, 0, &[]);
    write_chunk(out, CHUNK_END, &end)?;

    debug!(
        "[{}] encoded MVE: {}×{}, {} frames, {} blocks/frame, frame_dur={}µs",
        name,
        options.width,
        options.height,
        frames.len(),
        n_blocks,
        options.frame_duration_us
    );
    Ok(())
}

// ─── chunk builders ──────────────────────────────────────────────────────────

fn validate_dims(width: u16, height: u16) -> Result<()> {
    if width == 0 || height == 0 || width % 8 != 0 || height % 8 != 0 {
        return Err(MveEncodeError::InvalidDimensions { width, height });
    }
    Ok(())
}

fn validate_audio(audio: &AudioOptions) -> Result<()> {
    if audio.sample_rate > u16::MAX as u32 {
        return Err(MveEncodeError::AudioSampleRateTooHigh(audio.sample_rate));
    }
    if audio.channels != 1 && audio.channels != 2 {
        return Err(MveEncodeError::AudioChannelsInvalid(audio.channels));
    }
    Ok(())
}

fn build_init_audio_chunk(audio: &AudioOptions) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    // OC_AUDIO_BUFFERS v1 — 10-byte payload:
    //   u16 reserved, u16 flags, u16 sample_rate, u32 min_buf
    //
    // Audio is always Interplay DPCM. The decoder honours the
    // COMPRESSED flag whenever segment version is > 0; v1 matches
    // what avi2mve and real game cutscenes ship.
    let mut payload = Vec::with_capacity(10);
    payload.extend_from_slice(&0u16.to_le_bytes());
    let mut flags: u16 = AUDIO_FLAG_16BIT | AUDIO_FLAG_COMPRESSED;
    if audio.channels == 2 {
        flags |= AUDIO_FLAG_STEREO;
    }
    payload.extend_from_slice(&flags.to_le_bytes());
    payload.extend_from_slice(&(audio.sample_rate as u16).to_le_bytes());
    // Conventional min-buffer hint (mirrors what avi2mve writes); the
    // decoder reads but ignores the value.
    payload.extend_from_slice(&0x0001_0000u32.to_le_bytes());
    write_segment(&mut buf, OC_AUDIO_BUFFERS, 1, &payload);

    write_segment(&mut buf, OC_END_OF_CHUNK, 0, &[]);

    if buf.len() > u16::MAX as usize {
        return Err(MveEncodeError::ChunkTooBig);
    }
    Ok(buf)
}

fn build_init_video_chunk(
    width: u16,
    height: u16,
    frame_duration_us: u32,
    palette: &[[u8; 3]; 256],
) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    // OC_CREATE_TIMER — frame_duration_us = rate × subdiv (subdiv=1).
    let mut timer = Vec::with_capacity(6);
    timer.extend_from_slice(&frame_duration_us.to_le_bytes());
    timer.extend_from_slice(&1u16.to_le_bytes());
    write_segment(&mut buf, OC_CREATE_TIMER, 0, &timer);

    // OC_VIDEO_MODE — screen mode (matches what avi2mve writes).
    let mut mode = Vec::with_capacity(6);
    mode.extend_from_slice(&640u16.to_le_bytes());
    mode.extend_from_slice(&480u16.to_le_bytes());
    mode.extend_from_slice(&0u16.to_le_bytes());
    write_segment(&mut buf, OC_VIDEO_MODE, 0, &mode);

    // OC_VIDEO_BUFFERS v2 — frame size in 8×8 blocks, 8-bit Palette8.
    let mut buffers = Vec::with_capacity(8);
    buffers.extend_from_slice(&((width / 8) as u16).to_le_bytes());
    buffers.extend_from_slice(&((height / 8) as u16).to_le_bytes());
    buffers.extend_from_slice(&1u16.to_le_bytes());
    buffers.extend_from_slice(&0u16.to_le_bytes());
    write_segment(&mut buf, OC_VIDEO_BUFFERS, 2, &buffers);

    // OC_PALETTE — 4-byte header (start=0, count=256) + 768 bytes of
    // 6-bit RGB triples.
    let mut pal = Vec::with_capacity(4 + 768);
    pal.extend_from_slice(&0u16.to_le_bytes());
    pal.extend_from_slice(&256u16.to_le_bytes());
    for [r, g, b] in palette.iter() {
        pal.push(r >> 2);
        pal.push(g >> 2);
        pal.push(b >> 2);
    }
    write_segment(&mut buf, OC_PALETTE, 0, &pal);

    write_segment(&mut buf, OC_END_OF_CHUNK, 0, &[]);

    if buf.len() > u16::MAX as usize {
        return Err(MveEncodeError::ChunkTooBig);
    }
    Ok(buf)
}

fn build_frame_chunk(
    curr: &[u8],
    prev: Option<&[u8]>,
    stride: usize,
    bw: usize,
    bh: usize,
    n_blocks: usize,
    use_delta: bool,
    lossy_downsample: bool,
    audio: Option<(AudioOptions, &[i16])>,
    seq: u16,
) -> Result<Vec<u8>> {
    // Walk every block, decide its mode + payload.
    let height = bh * 8;
    let mut opcodes = Vec::with_capacity(n_blocks);
    let mut payload = Vec::new();
    for by in 0..bh {
        for bx in 0..bw {
            let curr_block = read_block(curr, stride, bx, by);
            let (opcode, mut bytes) = encode_block(
                &curr_block,
                prev,
                stride,
                height,
                bx,
                by,
                lossy_downsample,
            );
            opcodes.push(opcode);
            payload.append(&mut bytes);
        }
    }

    // Pack opcodes two-per-byte (low nibble first, high nibble
    // second). For odd counts the trailing nibble is zero (the
    // decoder only consults bytes for the blocks it iterates over,
    // matching `bx_count * by_count`, so a trailing zero is benign).
    let code_map_bytes = n_blocks.div_ceil(2);
    let mut code_map = vec![0u8; code_map_bytes];
    for (i, op) in opcodes.iter().enumerate() {
        if i & 1 == 0 {
            code_map[i >> 1] |= op & 0x0f;
        } else {
            code_map[i >> 1] |= (op & 0x0f) << 4;
        }
    }

    let mut buf = Vec::new();
    write_segment(&mut buf, OC_CODE_MAP, 0, &code_map);

    // OC_AUDIO_DATA (before the video, matching avi2mve's ordering):
    // 6-byte header + DPCM payload. `audio_size` in the header is
    // the *uncompressed* sample-byte count (n_samples * 2) because
    // the decoder uses it to size its output buffer; the bytes that
    // follow are the DPCM seeds + delta stream.
    if let Some((aopts, samples)) = audio {
        let uncompressed_bytes = samples.len() * 2;
        let payload_bytes = dpcm::compress(samples, aopts.channels);
        if payload_bytes.len() > u16::MAX as usize - 6 {
            return Err(MveEncodeError::ChunkTooBig);
        }
        let mut audio_payload = Vec::with_capacity(6 + payload_bytes.len());
        audio_payload.extend_from_slice(&seq.to_le_bytes());
        audio_payload.extend_from_slice(&DEFAULT_AUDIO_STREAM.to_le_bytes());
        audio_payload.extend_from_slice(&(uncompressed_bytes as u16).to_le_bytes());
        audio_payload.extend_from_slice(&payload_bytes);
        write_segment(&mut buf, OC_AUDIO_DATA, 0, &audio_payload);
    }

    let mut video = Vec::with_capacity(14 + payload.len());
    video.extend_from_slice(&[0u8; 12]);
    let flags: u16 = if use_delta { VIDEO_FLAG_DELTA } else { 0 };
    video.extend_from_slice(&flags.to_le_bytes());
    video.extend_from_slice(&payload);
    write_segment(&mut buf, OC_VIDEO_DATA, 0, &video);

    write_segment(&mut buf, OC_PLAY_VIDEO, 0, &[]);
    write_segment(&mut buf, OC_END_OF_CHUNK, 0, &[]);

    if buf.len() > u16::MAX as usize {
        return Err(MveEncodeError::ChunkTooBig);
    }
    Ok(buf)
}

// ─── per-block analysis & encoding ───────────────────────────────────────────

/// 8×8 grid of palette indices, row-major.
type Block = [[u8; 8]; 8];

fn read_block(image: &[u8], stride: usize, bx: usize, by: usize) -> Block {
    let mut g = [[0u8; 8]; 8];
    let top_left = by * 8 * stride + bx * 8;
    for y in 0..8 {
        for x in 0..8 {
            g[y][x] = image[top_left + y * stride + x];
        }
    }
    g
}

/// Decide which mode (and bitstream payload) to use for one block.
/// Returns `(opcode, payload_bytes)`. Always succeeds: any block that
/// doesn't fit the smaller modes falls through to mode `0xb` (raw
/// 8×8, 64 bytes), which can encode any pixel layout losslessly —
/// or, when `lossy_downsample` is set, to mode `0xc` with a lossy
/// 2×2 downsample (16 bytes), trading fidelity for size.
///
/// `prev_full` is the previous frame's pixel buffer, needed for both
/// the same-position skip check (mode `0x0`) and the 16×16 motion
/// search (mode `0x4`). `stride`/`height` are the frame dimensions.
fn encode_block(
    curr: &Block,
    prev_full: Option<&[u8]>,
    stride: usize,
    height: usize,
    bx: usize,
    by: usize,
    lossy_downsample: bool,
) -> (u8, Vec<u8>) {
    // 1. Skip (copy from previous frame) — cheapest at 0 bytes.
    if let Some(prev) = prev_full {
        let p = read_block(prev, stride, bx, by);
        if &p == curr {
            return (BLOCK_COPY_PREV, Vec::new());
        }
    }

    // 2. Solid colour — 1 byte. Detect by scanning for a single
    //    distinct palette index.
    let first = curr[0][0];
    let mut all_same = true;
    'solid: for row in curr.iter() {
        for &p in row.iter() {
            if p != first {
                all_same = false;
                break 'solid;
            }
        }
    }
    if all_same {
        return (BLOCK_SOLID, vec![first]);
    }

    // 3. Motion compensation against previous frame (1 byte). Search
    //    the 16×16 window of offsets `(dx, dy) ∈ [-8, 7]²` for an
    //    exact match; lossless guarantee preserved.
    if let Some(prev) = prev_full {
        if let Some(b) = find_motion_match(curr, prev, stride, height, bx, by) {
            return (BLOCK_MOTION_PREV, vec![b]);
        }
    }

    // 4. 4×4 quadrants — 4 bytes. Each 4×4 corner of the 8×8 block
    //    must be uniform. (Implies ≤ 4 distinct colours.)
    let q = [curr[0][0], curr[0][4], curr[4][0], curr[4][4]];
    let mut quadrants_uniform = true;
    'quad: for y in 0..8 {
        for x in 0..8 {
            let qi = ((y >= 4) as usize) * 2 + ((x >= 4) as usize);
            if curr[y][x] != q[qi] {
                quadrants_uniform = false;
                break 'quad;
            }
        }
    }
    if quadrants_uniform {
        return (BLOCK_QUADRANTS, q.to_vec());
    }

    // Count distinct colours, capped at 5 (we need to distinguish 2,
    // 3, 4, and "≥ 5" cases).
    let mut distinct: Vec<u8> = Vec::with_capacity(5);
    'colours: for row in curr.iter() {
        for &p in row.iter() {
            if !distinct.contains(&p) {
                distinct.push(p);
                if distinct.len() > 4 {
                    break 'colours;
                }
            }
        }
    }

    // 5. Two-colour delta pattern. Try 2×2-mask compact (4 bytes)
    //    first; fall back to 8-row full mask (10 bytes).
    if distinct.len() == 2 {
        let (a, b) = (distinct[0], distinct[1]);
        let lo = a.min(b);
        let hi = a.max(b);

        if let Some(mask16) = build_2x2_mask(curr, lo) {
            // Compact: decoder branches on `p0 > p1`. Write hi
            // first (p0), lo second (p1); the mask's '1' bits
            // mark `p1 = lo`.
            let mut bytes = Vec::with_capacity(4);
            bytes.push(hi);
            bytes.push(lo);
            bytes.extend_from_slice(&mask16.to_le_bytes());
            return (BLOCK_DELTA, bytes);
        }

        // Full per-row mask: decoder branches on `p0 <= p1`. Write
        // lo first (p0), hi second (p1); each row mask's bit-x is
        // set for pixels equal to `p1 = hi`.
        let row_masks = build_row_masks(curr, hi);
        let mut bytes = Vec::with_capacity(10);
        bytes.push(lo);
        bytes.push(hi);
        bytes.extend_from_slice(&row_masks);
        return (BLOCK_DELTA, bytes);
    }

    // 6. Quad-pattern (0x9) — 3-4 distinct colours. Picks the
    //    cheapest of four sub-modes: per-2×2 (8 bytes), per-2×1
    //    wide / per-1×2 tall (12 bytes each), per-pixel (20 bytes).
    //    Mode 0x8's 12-byte half-split and 16-byte per-quadrant
    //    forms slot in *before* the 20-byte per-pixel fall-through.
    if distinct.len() == 3 || distinct.len() == 4 {
        distinct.sort_unstable();
        if is_2x2_uniform(curr) {
            return (BLOCK_QUAD_PATTERN, build_0x9_per_2x2(curr, &distinct));
        }
        if is_2x1_uniform(curr) {
            return (BLOCK_QUAD_PATTERN, build_0x9_per_2x1_wide(curr, &distinct));
        }
        if is_1x2_uniform(curr) {
            return (BLOCK_QUAD_PATTERN, build_0x9_per_1x2_tall(curr, &distinct));
        }
        if let Some(payload) = build_0x8_vertical_halves(curr)
            .or_else(|| build_0x8_horizontal_halves(curr))
        {
            return (BLOCK_QUADRANT_PAIRS, payload);
        }
        if let Some(payload) = build_0x8_per_quadrant(curr) {
            return (BLOCK_QUADRANT_PAIRS, payload);
        }
        return (BLOCK_QUAD_PATTERN, build_0x9_per_pixel(curr, &distinct));
    }

    // 7. ≥ 5 distinct colours. Cheapest applicable mode:
    //    0xc (16 B if every 2×2 uniform) → 0x8 per-quadrant (16 B,
    //    needs ≤ 2 colours/quadrant; max 8 distinct) →
    //    0xa half-split (24 B) → 0xa per-quadrant (32 B) →
    //    fallback to 0xb (or lossy 0xc).
    //
    // 0x8 half-split caps at 4 distinct total (≤ 2 per half × 2
    // halves), so it can never fire here — only `…per_quadrant`.
    if let Some(grid) = build_4x4_fill(curr) {
        return (BLOCK_4X4_FILL, grid);
    }
    if let Some(payload) = build_0x8_per_quadrant(curr) {
        return (BLOCK_QUADRANT_PAIRS, payload);
    }
    if let Some(payload) = build_0xa_vertical_halves(curr)
        .or_else(|| build_0xa_horizontal_halves(curr))
    {
        return (BLOCK_QUADRANT_QUADS, payload);
    }
    if let Some(payload) = build_0xa_per_quadrant(curr) {
        return (BLOCK_QUADRANT_QUADS, payload);
    }

    // 8a. Lossy fallback — emit `0xc` with 2×2 downsample (16 bytes)
    //     instead of `0xb` raw (64 bytes). Top-left of each 2×2 wins.
    if lossy_downsample {
        return (BLOCK_4X4_FILL, build_4x4_fill_downsampled(curr));
    }

    // 8b. Raw 8×8 (0xb) — 64 bytes. Always works, lossless.
    let mut bytes = Vec::with_capacity(64);
    for row in curr.iter() {
        bytes.extend_from_slice(row);
    }
    (BLOCK_RAW, bytes)
}

/// Lossy 2×2-downsample variant of `build_4x4_fill`: always succeeds
/// by taking the top-left pixel of each 2×2 sub-block as the
/// representative colour.
fn build_4x4_fill_downsampled(block: &Block) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            out.push(block[y][x]);
            x += 2;
        }
        y += 2;
    }
    out
}

/// Brute-force search the 16×16 motion window for an exact match of
/// `curr` in `prev`. Offsets `(dx, dy) ∈ [-8, 7]²`; out-of-bounds
/// candidates are skipped. Returns the encoded byte
/// `((dy + 8) << 4) | (dx + 8)` of the first match found, or `None`.
fn find_motion_match(
    curr: &Block,
    prev: &[u8],
    stride: usize,
    height: usize,
    bx: usize,
    by: usize,
) -> Option<u8> {
    let bx_pix = bx as isize * 8;
    let by_pix = by as isize * 8;
    for dy_field in 0i32..16 {
        let dy = dy_field - 8;
        let src_y = by_pix + dy as isize;
        if src_y < 0 || src_y as usize + 8 > height {
            continue;
        }
        for dx_field in 0i32..16 {
            let dx = dx_field - 8;
            let src_x = bx_pix + dx as isize;
            if src_x < 0 || src_x as usize + 8 > stride {
                continue;
            }
            let mut matches = true;
            for y in 0..8 {
                let row_start = (src_y as usize + y) * stride + src_x as usize;
                if prev[row_start..row_start + 8] != curr[y][..] {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some(((dy_field as u8) << 4) | (dx_field as u8));
            }
        }
    }
    None
}

/// If every 2×2 sub-block of `block` is uniform, return the 16
/// sub-block colour bytes in the row-major order the decoder reads
/// them: (0,0), (0,2), (0,4), (0,6), (2,0), …, (6,6).
fn build_4x4_fill(block: &Block) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(16);
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = block[y][x];
            if block[y][x + 1] != v
                || block[y + 1][x] != v
                || block[y + 1][x + 1] != v
            {
                return None;
            }
            out.push(v);
            x += 2;
        }
        y += 2;
    }
    Some(out)
}

/// Build a 16-bit "1 = `target_colour`" mask over the 16 2×2 sub-blocks
/// of `block`, in the iteration order the decoder uses (row-major,
/// LSB-first). Returns `None` if any 2×2 sub-block is not uniform.
fn build_2x2_mask(block: &Block, target_colour: u8) -> Option<u16> {
    let mut mask: u16 = 0;
    let mut bit: u16 = 1;
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = block[y][x];
            if block[y][x + 1] != v
                || block[y + 1][x] != v
                || block[y + 1][x + 1] != v
            {
                return None;
            }
            if v == target_colour {
                mask |= bit;
            }
            bit <<= 1;
            x += 2;
        }
        y += 2;
    }
    Some(mask)
}

/// 8 per-row 8-bit masks where bit `x` of row `y` is set if pixel
/// `(x, y)` equals `target_colour`.
fn build_row_masks(block: &Block, target_colour: u8) -> [u8; 8] {
    let mut rows = [0u8; 8];
    for y in 0..8 {
        let mut row = 0u8;
        for x in 0..8 {
            if block[y][x] == target_colour {
                row |= 1 << x;
            }
        }
        rows[y] = row;
    }
    rows
}

// ─── 0x9 quad-pattern helpers ────────────────────────────────────────────────
//
// Mode 0x9 carries 4 palette bytes `p[0..4]` and a per-sub-block mask;
// the decoder branches on the byte ordering of `p`:
//
// - per-pixel    (`p0 ≤ p1 && p2 ≤ p3`): 16-byte mask, 8 sub-blocks/row × 8 rows
// - per-2×2     (`p0 ≤ p1 && p2 > p3`): 4-byte mask, 16 sub-blocks
// - per-2×1 wide (`p0 > p1 && p2 ≤ p3`): 8-byte mask, 32 sub-blocks (2 wide × 1 tall)
// - per-1×2 tall (`p0 > p1 && p2 > p3`): 8-byte mask, 32 sub-blocks (1 wide × 2 tall)
//
// The encoder fixes the `p` order so the right branch fires; the
// `*_palette_*` helpers below produce a 4-tuple from the block's
// distinct colours sorted ascending.

fn is_2x2_uniform(block: &Block) -> bool {
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = block[y][x];
            if block[y][x + 1] != v
                || block[y + 1][x] != v
                || block[y + 1][x + 1] != v
            {
                return false;
            }
            x += 2;
        }
        y += 2;
    }
    true
}

fn is_2x1_uniform(block: &Block) -> bool {
    for y in 0..8 {
        let mut x = 0;
        while x < 8 {
            if block[y][x] != block[y][x + 1] {
                return false;
            }
            x += 2;
        }
    }
    true
}

fn is_1x2_uniform(block: &Block) -> bool {
    let mut y = 0;
    while y < 8 {
        for x in 0..8 {
            if block[y][x] != block[y + 1][x] {
                return false;
            }
        }
        y += 2;
    }
    true
}

/// `distinct` must be sorted ascending with len ∈ {3, 4}.
fn quad_palette_per_pixel(distinct: &[u8]) -> [u8; 4] {
    match distinct.len() {
        3 => [distinct[0], distinct[1], distinct[2], distinct[2]],
        4 => [distinct[0], distinct[1], distinct[2], distinct[3]],
        _ => unreachable!(),
    }
}

fn quad_palette_per_2x2(distinct: &[u8]) -> [u8; 4] {
    match distinct.len() {
        3 => [distinct[0], distinct[1], distinct[2], distinct[1]],
        4 => [distinct[0], distinct[1], distinct[3], distinct[2]],
        _ => unreachable!(),
    }
}

fn quad_palette_per_2x1_wide(distinct: &[u8]) -> [u8; 4] {
    match distinct.len() {
        3 => [distinct[1], distinct[0], distinct[1], distinct[2]],
        4 => [distinct[1], distinct[0], distinct[2], distinct[3]],
        _ => unreachable!(),
    }
}

fn quad_palette_per_1x2_tall(distinct: &[u8]) -> [u8; 4] {
    match distinct.len() {
        3 => [distinct[1], distinct[0], distinct[2], distinct[1]],
        4 => [distinct[1], distinct[0], distinct[3], distinct[2]],
        _ => unreachable!(),
    }
}

#[inline]
fn colour_to_index(p: &[u8; 4], colour: u8) -> u32 {
    p.iter().position(|&v| v == colour).expect("colour must be in p[]") as u32
}

fn build_0x9_per_2x2(block: &Block, distinct: &[u8]) -> Vec<u8> {
    let p = quad_palette_per_2x2(distinct);
    let mut out = Vec::with_capacity(8);
    out.extend_from_slice(&p);
    let mut flags: u32 = 0;
    let mut shifter = 0;
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let idx = colour_to_index(&p, block[y][x]);
            flags |= idx << shifter;
            shifter += 2;
            x += 2;
        }
        y += 2;
    }
    out.extend_from_slice(&flags.to_le_bytes());
    out
}

fn build_0x9_per_2x1_wide(block: &Block, distinct: &[u8]) -> Vec<u8> {
    let p = quad_palette_per_2x1_wide(distinct);
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&p);
    let mut y = 0;
    while y < 8 {
        let mut flags: u32 = 0;
        let mut shifter = 0;
        for dy in 0..4usize {
            let mut x = 0;
            while x < 8 {
                let idx = colour_to_index(&p, block[y + dy][x]);
                flags |= idx << shifter;
                shifter += 2;
                x += 2;
            }
        }
        out.extend_from_slice(&flags.to_le_bytes());
        y += 4;
    }
    out
}

fn build_0x9_per_1x2_tall(block: &Block, distinct: &[u8]) -> Vec<u8> {
    let p = quad_palette_per_1x2_tall(distinct);
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&p);
    let mut y = 0;
    while y < 8 {
        let mut flags: u32 = 0;
        let mut shifter = 0;
        let mut dy = 0;
        while dy < 4 {
            for x in 0..8 {
                let idx = colour_to_index(&p, block[y + dy][x]);
                flags |= idx << shifter;
                shifter += 2;
            }
            dy += 2;
        }
        out.extend_from_slice(&flags.to_le_bytes());
        y += 4;
    }
    out
}

fn build_0x9_per_pixel(block: &Block, distinct: &[u8]) -> Vec<u8> {
    let p = quad_palette_per_pixel(distinct);
    let mut out = Vec::with_capacity(20);
    out.extend_from_slice(&p);
    for y in 0..8 {
        let mut flags: u16 = 0;
        for x in 0..8 {
            let idx = colour_to_index(&p, block[y][x]) as u16;
            flags |= idx << (x * 2);
        }
        out.extend_from_slice(&flags.to_le_bytes());
    }
    out
}

// ─── 0x8 quadrant-pair / half-split helpers ─────────────────────────────────
//
// Mode 0x8 carries 8 mask bytes `b[0..8]` and either 8 palette bytes
// `p[0..8]` (per-quadrant, 16 B total) or 4 palette bytes `p[0..4]`
// (per-half, 12 B total). Sub-mode is selected by the decoder via
//
//   if p[0] <= p[1]              → per-quadrant   (16 B)
//   else if p[2] <= p[3]         → vertical halves (12 B)
//   else                         → horizontal halves (12 B)
//
// Per-quadrant: each 4×4 quadrant gets its own (pp0, pp1) pair and
// a 16-bit mask that picks between them. Bit 0 → pp0, 1 → pp1.
// Quadrant → palette / mask layout (matches `pack_flags_8` indexing):
//
//   top-left     → p[0..2]  b[0..2]
//   bottom-left  → p[2..4]  b[2..4]
//   top-right    → p[4..6]  b[4..6]
//   bottom-right → p[6..8]  b[6..8]
//
// Each quadrant's two mask bytes hold rows (0,1) and (2,3) packed as
// low/high nibbles: `b[lo].low` = row 0 bits 0..3, `b[lo].high` =
// row 1 bits 0..3, `b[hi].low` = row 2, `b[hi].high` = row 3.
//
// Vertical halves (per-half): left half (x<4) uses p[0]/p[1], right
// half (x≥4) uses p[2]/p[3]. Mask layout is identical to per-quadrant
// (decoder calls the same `pack_flags_8`), but only two palette
// pairs apply across all rows of each side.
//
// Horizontal halves: top half (y<4) uses p[0]/p[1], bottom half (y≥4)
// uses p[2]/p[3]. Mask is per-row 8-bit, b[y] bit x = pixel (y, x).

fn build_0x8_per_quadrant(block: &Block) -> Option<Vec<u8>> {
    // Quadrants: ((origin_y, origin_x), palette_slot, mask_slot)
    let quads = [
        ((0usize, 0usize), 0usize),
        ((4, 0), 2),
        ((0, 4), 4),
        ((4, 4), 6),
    ];
    let mut p = [0u8; 8];
    let mut b = [0u8; 8];
    for &((qy, qx), slot) in quads.iter() {
        let (pp0, pp1) = pick_quadrant_pair_ascending(block, qy, qx)?;
        p[slot] = pp0;
        p[slot + 1] = pp1;
        let (lo, hi) = build_quadrant_mask_2col(block, qy, qx, pp0, pp1);
        b[slot] = lo;
        b[slot + 1] = hi;
    }
    // Branch selector: top-left needs p[0] <= p[1]. `pick_…ascending`
    // guarantees that.
    let mut out = Vec::with_capacity(16);
    out.push(p[0]);
    out.push(p[1]);
    out.push(b[0]);
    out.push(b[1]);
    for &slot in &[2usize, 4, 6] {
        out.push(p[slot]);
        out.push(p[slot + 1]);
        out.push(b[slot]);
        out.push(b[slot + 1]);
    }
    Some(out)
}

fn build_0x8_vertical_halves(block: &Block) -> Option<Vec<u8>> {
    // Left half: x in 0..4 across all 8 rows. Right half: x in 4..8.
    let left = collect_distinct(block, 0, 0, 8, 4, 2)?;
    let right = collect_distinct(block, 0, 4, 8, 8, 2)?;
    // Branch: p[0] > p[1] AND p[2] <= p[3].
    let (p0, p1) = pick_pair_descending(&left)?;
    let (p2, p3) = pick_pair_ascending(&right);
    let mut b = [0u8; 8];
    write_vertical_halves_mask(block, &mut b, p0, p1, p2, p3);
    let mut out = Vec::with_capacity(12);
    out.push(p0);
    out.push(p1);
    out.push(b[0]);
    out.push(b[1]);
    out.push(b[2]);
    out.push(b[3]);
    out.push(p2);
    out.push(p3);
    out.push(b[4]);
    out.push(b[5]);
    out.push(b[6]);
    out.push(b[7]);
    Some(out)
}

fn build_0x8_horizontal_halves(block: &Block) -> Option<Vec<u8>> {
    // Top half: y in 0..4 across all 8 cols. Bottom half: y in 4..8.
    let top = collect_distinct(block, 0, 0, 4, 8, 2)?;
    let bot = collect_distinct(block, 4, 0, 8, 8, 2)?;
    // Branch: p[0] > p[1] AND p[2] > p[3].
    let (p0, p1) = pick_pair_descending(&top)?;
    let (p2, p3) = pick_pair_descending(&bot)?;
    // Per-row mask: b[y] = 8-bit row mask, bit x = pixel(y, x).
    let mut b = [0u8; 8];
    for y in 0..8 {
        let (pp0, pp1) = if y < 4 { (p0, p1) } else { (p2, p3) };
        let mut row = 0u8;
        for x in 0..8 {
            let v = block[y][x];
            let bit = if v == pp1 { 1u8 } else if v == pp0 { 0 } else {
                // unreachable: collect_distinct already ensured ≤ 2
                // colours per half, and pick_pair_descending always
                // returns one of them as pp1.
                return None;
            };
            row |= bit << x;
        }
        b[y] = row;
    }
    let mut out = Vec::with_capacity(12);
    out.push(p0);
    out.push(p1);
    out.push(b[0]);
    out.push(b[1]);
    out.push(b[2]);
    out.push(b[3]);
    out.push(p2);
    out.push(p3);
    out.push(b[4]);
    out.push(b[5]);
    out.push(b[6]);
    out.push(b[7]);
    Some(out)
}

/// Pack the 16 bits of one 4×4 quadrant into two bytes (low-nibble =
/// first row of the pair, high-nibble = second row). Bit value is
/// `1` when the source pixel matches `pp1`, else `0`.
fn build_quadrant_mask_2col(
    block: &Block,
    qy: usize,
    qx: usize,
    pp0: u8,
    pp1: u8,
) -> (u8, u8) {
    let mut lo = 0u8;
    let mut hi = 0u8;
    for dy in 0..4 {
        for dx in 0..4 {
            let v = block[qy + dy][qx + dx];
            let bit: u8 = if v == pp1 {
                1
            } else {
                debug_assert_eq!(v, pp0);
                0
            };
            match dy {
                0 => lo |= bit << dx,
                1 => lo |= bit << (4 + dx),
                2 => hi |= bit << dx,
                _ => hi |= bit << (4 + dx),
            }
        }
    }
    (lo, hi)
}

/// Compute the 8 mask bytes for the 0x8 vertical-halves sub-mode. Bit
/// layout matches `pack_flags_8`: each byte holds two half-rows of
/// one half-column block (4 bits each, low/high nibble). Horizontal
/// halves use a different per-row packing handled inline by the
/// caller.
fn write_vertical_halves_mask(
    block: &Block,
    b: &mut [u8; 8],
    p0: u8,
    p1: u8,
    p2: u8,
    p3: u8,
) {
    for y in 0..8 {
        for x in 0..8 {
            let v = block[y][x];
            let (pp0, pp1) = if x < 4 { (p0, p1) } else { (p2, p3) };
            let bit: u8 = if v == pp1 {
                1
            } else {
                debug_assert_eq!(v, pp0);
                0
            };
            let byte_index = match (y < 4, x < 4) {
                (true, true) => y / 2,
                (true, false) => 4 + y / 2,
                (false, true) => 2 + (y - 4) / 2,
                (false, false) => 6 + (y - 4) / 2,
            };
            let bit_in_byte = (y % 2) * 4 + (x % 4);
            b[byte_index] |= bit << bit_in_byte;
        }
    }
}

/// Find the up-to-2 distinct palette indices in `block[y0..y1, x0..x1]`.
/// Returns `None` if there are more than `max` distinct values.
fn collect_distinct(
    block: &Block,
    y0: usize,
    x0: usize,
    y1: usize,
    x1: usize,
    max: usize,
) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(max + 1);
    for y in y0..y1 {
        for x in x0..x1 {
            let v = block[y][x];
            if !out.contains(&v) {
                out.push(v);
                if out.len() > max {
                    return None;
                }
            }
        }
    }
    Some(out)
}

fn pick_quadrant_pair_ascending(block: &Block, qy: usize, qx: usize) -> Option<(u8, u8)> {
    let colours = collect_distinct(block, qy, qx, qy + 4, qx + 4, 2)?;
    Some(pick_pair_ascending(&colours))
}

/// `colours.len() ∈ {1, 2}`. Returns `(pp0, pp1)` with `pp0 ≤ pp1`,
/// duplicating the single colour when there's only one.
fn pick_pair_ascending(colours: &[u8]) -> (u8, u8) {
    match colours.len() {
        1 => (colours[0], colours[0]),
        2 => {
            let (a, b) = (colours[0], colours[1]);
            (a.min(b), a.max(b))
        }
        _ => unreachable!("collect_distinct caps at 2"),
    }
}

/// `colours.len() ∈ {1, 2}`. Returns `(pp0, pp1)` with `pp0 > pp1`
/// strictly. The single-colour case fabricates a phantom second
/// palette entry adjacent to `v` so the inequality holds while the
/// effective decoded value remains `v`.
fn pick_pair_descending(colours: &[u8]) -> Option<(u8, u8)> {
    match colours.len() {
        1 => {
            let v = colours[0];
            // Want all decoded pixels = v. Encoder side picks `bit = 1`
            // when the pixel equals `pp1` else `bit = 0`; whichever
            // slot we put `v` in, the other slot just has to satisfy
            // the strict inequality — and never appear in the source
            // pixels.
            if v > 0 {
                // pp0 = v, pp1 = v - 1: bits 0 → pp0 = v.
                Some((v, v - 1))
            } else {
                // v == 0: pp0 = 1, pp1 = 0: bits 1 → pp1 = 0 = v.
                Some((1, 0))
            }
        }
        2 => {
            let (a, b) = (colours[0], colours[1]);
            Some((a.max(b), a.min(b)))
        }
        _ => unreachable!("collect_distinct caps at 2"),
    }
}

// ─── 0xa quadrant-quads / half-split helpers ────────────────────────────────
//
// Mode 0xa carries up to 16 palette bytes and 16 mask bytes. Sub-modes
// are selected by the decoder:
//
//   if p[0] <= p[1]              → 4 colours per quadrant   (32 B)
//   else if p[4] <= p[5]         → 4 colours per vertical half (24 B)
//   else                         → 4 colours per horizontal half (24 B)
//
// Per-quadrant palette layout: p[0..4] = top-left, p[4..8] =
// bottom-left, p[8..12] = top-right, p[12..16] = bottom-right. Mask
// byte b[N] holds row R of one 4-pixel-wide column-strip (2 bits per
// pixel × 4 = 8 bits). Indexing matches the decoder's
// `flags = (b[y+8] << 8) | b[y]` and `idx = split + lower + (flags >>
// 2x) & 3`:
//
//   b[0..4]  = top-left  rows 0..3 (x<4)
//   b[4..8]  = bottom-left rows 4..7 (x<4)
//   b[8..12] = top-right rows 0..3 (x≥4)
//   b[12..16]= bottom-right rows 4..7 (x≥4)
//
// Vertical-halves palette layout: p[0..4] = left, p[4..8] = right.
// Mask: b[y] for x<4 of row y, b[y+8] for x≥4 of row y.
//
// Horizontal-halves palette layout: p[0..4] = top, p[4..8] = bottom.
// Mask: b[2y] for x<4 of row y, b[2y+1] for x≥4 of row y.

fn build_0xa_per_quadrant(block: &Block) -> Option<Vec<u8>> {
    // (origin_y, origin_x, palette_slot, mask_slot_base) — for this
    // sub-mode palette and mask slots are aligned at the same offsets.
    let quads = [
        ((0usize, 0usize), 0usize),
        ((4, 0), 4),
        ((0, 4), 8),
        ((4, 4), 12),
    ];
    let mut p = [0u8; 16];
    let mut b = [0u8; 16];
    for &((qy, qx), slot) in quads.iter() {
        let colours = collect_distinct(block, qy, qx, qy + 4, qx + 4, 4)?;
        let (a, c, d, e) = pick_quad_ascending(&colours);
        p[slot] = a;
        p[slot + 1] = c;
        p[slot + 2] = d;
        p[slot + 3] = e;
        // Build per-row 8-bit masks (4 pixels × 2 bits) for this
        // quadrant. b[slot + dy] holds row dy of the quadrant.
        for dy in 0..4 {
            let mut row = 0u8;
            for dx in 0..4 {
                let v = block[qy + dy][qx + dx];
                let idx = palette_index_4(&p[slot..slot + 4], v);
                row |= (idx & 0x03) << (dx * 2);
            }
            b[slot + dy] = row;
        }
    }
    // Branch selector: p[0] <= p[1] holds because pick_quad_ascending
    // sorts ascending (and pads with the highest colour).
    let mut out = Vec::with_capacity(32);
    // Header: p[0..4], b[0..4]
    out.extend_from_slice(&p[0..4]);
    out.extend_from_slice(&b[0..4]);
    // Three more chunks: p[4..8], b[4..8] / p[8..12], b[8..12] / p[12..16], b[12..16]
    for &start in &[4usize, 8, 12] {
        out.extend_from_slice(&p[start..start + 4]);
        out.extend_from_slice(&b[start..start + 4]);
    }
    Some(out)
}

fn build_0xa_vertical_halves(block: &Block) -> Option<Vec<u8>> {
    let left = collect_distinct(block, 0, 0, 8, 4, 4)?;
    let right = collect_distinct(block, 0, 4, 8, 8, 4)?;
    // Branch: p[0] > p[1] AND p[4] <= p[5].
    let (p0, p1, p2, p3) = pick_quad_descending(&left)?;
    let (p4, p5, p6, p7) = pick_quad_ascending(&right);
    let p_left = [p0, p1, p2, p3];
    let p_right = [p4, p5, p6, p7];
    let mut b = [0u8; 16];
    for y in 0..8 {
        let mut left_mask = 0u8;
        for x in 0..4 {
            let idx = palette_index_4(&p_left, block[y][x]);
            left_mask |= (idx & 0x03) << (x * 2);
        }
        b[y] = left_mask;
        let mut right_mask = 0u8;
        for x in 4..8 {
            let idx = palette_index_4(&p_right, block[y][x]);
            right_mask |= (idx & 0x03) << ((x - 4) * 2);
        }
        b[y + 8] = right_mask;
    }
    Some(emit_0xa_halves(&p_left, &p_right, &b))
}

fn build_0xa_horizontal_halves(block: &Block) -> Option<Vec<u8>> {
    let top = collect_distinct(block, 0, 0, 4, 8, 4)?;
    let bot = collect_distinct(block, 4, 0, 8, 8, 4)?;
    // Branch: p[0] > p[1] AND p[4] > p[5].
    let (p0, p1, p2, p3) = pick_quad_descending(&top)?;
    let (p4, p5, p6, p7) = pick_quad_descending(&bot)?;
    let p_top = [p0, p1, p2, p3];
    let p_bot = [p4, p5, p6, p7];
    let mut b = [0u8; 16];
    for y in 0..8 {
        let pal = if y < 4 { &p_top } else { &p_bot };
        let mut left_mask = 0u8;
        for x in 0..4 {
            let idx = palette_index_4(pal, block[y][x]);
            left_mask |= (idx & 0x03) << (x * 2);
        }
        b[y * 2] = left_mask;
        let mut right_mask = 0u8;
        for x in 4..8 {
            let idx = palette_index_4(pal, block[y][x]);
            right_mask |= (idx & 0x03) << ((x - 4) * 2);
        }
        b[y * 2 + 1] = right_mask;
    }
    Some(emit_0xa_halves(&p_top, &p_bot, &b))
}

/// Emit a 24-byte 0xa half-split payload in the decoder's read order:
/// header `p[0..4] b[0..4]`, then `b[4..8]`, then `p_other[0..4]`,
/// then `b[8..16]`.
fn emit_0xa_halves(first_pal: &[u8; 4], second_pal: &[u8; 4], b: &[u8; 16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(first_pal);
    out.extend_from_slice(&b[0..4]);
    out.extend_from_slice(&b[4..8]);
    out.extend_from_slice(second_pal);
    out.extend_from_slice(&b[8..16]);
    out
}

/// `colours.len() ∈ {1, 2, 3, 4}`. Sort ascending and pad to 4 entries
/// by repeating the last colour. Caller looks up via the FIRST
/// matching position so duplicate slots are harmless.
fn pick_quad_ascending(colours: &[u8]) -> (u8, u8, u8, u8) {
    let mut s = colours.to_vec();
    s.sort_unstable();
    while s.len() < 4 {
        s.push(*s.last().unwrap());
    }
    (s[0], s[1], s[2], s[3])
}

/// `colours.len() ∈ {1, 2, 3, 4}`. Returns `(p0, p1, p2, p3)` with
/// `p0 > p1` strictly. Single-colour input fabricates a phantom
/// second palette slot the same way as `pick_pair_descending`.
fn pick_quad_descending(colours: &[u8]) -> Option<(u8, u8, u8, u8)> {
    if colours.is_empty() {
        return None;
    }
    if colours.len() == 1 {
        let v = colours[0];
        if v > 0 {
            return Some((v, v - 1, v, v));
        } else {
            return Some((1, 0, 0, 0));
        }
    }
    let mut s = colours.to_vec();
    s.sort_unstable_by(|a, b| b.cmp(a)); // descending
    while s.len() < 4 {
        s.push(*s.last().unwrap());
    }
    Some((s[0], s[1], s[2], s[3]))
}

#[inline]
fn palette_index_4(p: &[u8], v: u8) -> u8 {
    p.iter()
        .position(|&c| c == v)
        .expect("colour must be in palette") as u8
}

// ─── low-level emitters ──────────────────────────────────────────────────────

fn write_segment(out: &mut Vec<u8>, seg_type: u8, version: u8, payload: &[u8]) {
    let size = payload.len() as u16;
    out.extend_from_slice(&size.to_le_bytes());
    out.push(seg_type);
    out.push(version);
    out.extend_from_slice(payload);
}

fn write_chunk<W: Write>(out: &mut W, chunk_type: u16, body: &[u8]) -> io::Result<()> {
    let size = body.len() as u16;
    out.write_all(&size.to_le_bytes())?;
    out.write_all(&chunk_type.to_le_bytes())?;
    out.write_all(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_multiple_of_8() {
        let mut out = Vec::new();
        let err = encode_solid_colour_video(321, 240, [0, 0, 0], 1, 66_667, "", &mut out)
            .unwrap_err();
        assert!(matches!(err, MveEncodeError::InvalidDimensions { .. }));
    }

    #[test]
    fn rejects_zero_frame_duration() {
        let img = StaticImage {
            width: 8,
            height: 8,
            pixels: vec![0u8; 64],
            palette: Box::new([[0u8; 3]; 256]),
        };
        let mut out = Vec::new();
        let err = encode_static_palette8(&img, 1, 0, "", &mut out).unwrap_err();
        assert!(matches!(err, MveEncodeError::InvalidFrameDuration(0)));
    }

    #[test]
    fn five_colour_block_now_succeeds_via_raw() {
        // 8×8 block with 5 distinct indices: Phase-2 used to reject;
        // Phase-3 falls through to 0xb (raw).
        let mut pixels = vec![1u8; 64];
        pixels[0] = 2;
        pixels[1] = 3;
        pixels[2] = 4;
        pixels[3] = 5;
        let img = StaticImage {
            width: 8,
            height: 8,
            pixels,
            palette: Box::new([[0u8; 3]; 256]),
        };
        let mut out = Vec::new();
        encode_static_palette8(&img, 1, 66_667, "", &mut out).unwrap();
    }

    #[test]
    fn three_colours_not_quadrants_succeeds_via_4x4_or_raw() {
        // 3 distinct colours, not arranged as 4×4 quadrants.
        // Phase-2 used to reject; Phase-3 emits either 0xc or 0xb.
        let mut pixels = vec![1u8; 64];
        pixels[0] = 2;
        pixels[63] = 3;
        let img = StaticImage {
            width: 8,
            height: 8,
            pixels,
            palette: Box::new([[0u8; 3]; 256]),
        };
        let mut out = Vec::new();
        encode_static_palette8(&img, 1, 66_667, "", &mut out).unwrap();
    }

    #[test]
    fn produced_signature_is_valid() {
        let mut out = Vec::new();
        encode_solid_colour_video(8, 8, [0, 0, 0], 1, 66_667, "", &mut out).unwrap();
        assert_eq!(&out[..18], b"Interplay MVE File");
        assert_eq!(&out[18..23], b"\x1a\x00\x1a\x00\x00");
    }
}
