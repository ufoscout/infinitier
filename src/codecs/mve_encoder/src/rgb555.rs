#![allow(clippy::needless_range_loop)]

//! RGB555 (HiColor / 16-bit) MVE encoder.
//!
//! Parallel to the 8-bit paletted path in `lib.rs`. Output bitstream
//! mirrors what `mcomp.exe HICOLOR.CFG` writes:
//!
//! - `OC_VIDEO_MODE`    flags = `0x0110` (display-mode hint for HiColor)
//! - `OC_VIDEO_BUFFERS` v2 with `format_flag = 1` (16 bpp)
//! - **No** `OC_PALETTE` segment
//! - Per-frame `OC_VIDEO_DATA` payload uses two sub-streams: a
//!   `u16 LE` "motion-stream offset" header followed by the colour
//!   stream (modes `0x5`, `0x7..0xf`) and then the motion-vector
//!   stream (modes `0x2`, `0x3`, `0x4`).
//!
//! ## Block-mode chooser (cost-sorted)
//!
//! | Step | Mode                     | Bytes | Constraint |
//! |---|---|---|---|
//! |  1 | `0x0` skip                | 0   | matches prev frame at same offset |
//! |  2 | `0xe` solid               | 2   | every pixel identical |
//! |  3 | `0x4` motion              | 1*  | exact match within ±8 px (\* in motion stream) |
//! |  4 | `0x7` per-2×2 (2-colour)  | 6   | 2 colours, every 2×2 uniform |
//! |  5 | `0xd` quadrants           | 8   | each 4×4 quadrant uniform |
//! |  6 | `0x7` per-row (2-colour)  | 12  | 2 colours arbitrary |
//! |  7 | `0x9` per-2×2 (3-4 col.)  | 12  | 3-4 colours, every 2×2 uniform |
//! |  8 | `0x9` per-2×1 / per-1×2   | 16  | 3-4 colours + 2×1/1×2 uniformity |
//! |  9 | `0x8` half-split          | 16  | each half ≤ 2 colours |
//! | 10 | `0x9` per-pixel           | 24  | 3-4 colours arbitrary |
//! | 11 | `0x8` per-quadrant        | 24  | each 4×4 quadrant ≤ 2 colours |
//! | 12 | `0xa` half-split          | 32  | each half ≤ 4 colours |
//! | 13 | `0xc` 4×4 fill            | 32  | every 2×2 uniform (any colour count) |
//! | 14 | `0xa` per-quadrant        | 48  | each 4×4 quadrant ≤ 4 colours |
//! | 15 | `0xb` raw                 | 128 | always |
//!
//! ## Sub-mode selector — bit 15 of the first u16
//!
//! Modes `0x7`/`0x8`/`0x9`/`0xa` read certain palette positions and
//! interpret bit 15 as a sub-mode selector. The decoder strips bit 15
//! from those positions after reading, so any colour we put there
//! must have bit 15 = 0 in the source — otherwise the round-trip
//! loses one bit. `pack_rgb555` always produces values in [0, 0x7fff],
//! so this is automatic for normal usage; the chooser still skips
//! these modes if it sees a pixel with bit 15 set, falling back to
//! the bit-15-safe modes `0x0`/`0xe`/`0x4`/`0xd`/`0xc`/`0xb`.

use std::io::Write;

use crate::{
    AudioOptions, MveEncodeError, Result, write_chunk, write_segment,
    AUDIO_FLAG_16BIT, AUDIO_FLAG_COMPRESSED, AUDIO_FLAG_STEREO, CHUNK_END, CHUNK_FRAME,
    CHUNK_INIT_AUDIO, CHUNK_INIT_VIDEO, DEFAULT_AUDIO_STREAM, OC_AUDIO_BUFFERS, OC_AUDIO_DATA,
    OC_CODE_MAP, OC_CREATE_TIMER, OC_END_OF_CHUNK, OC_END_OF_STREAM, OC_PLAY_VIDEO,
    OC_VIDEO_BUFFERS, OC_VIDEO_DATA, OC_VIDEO_MODE, SIGNATURE_24, SIGNATURE_TAIL,
    VIDEO_FLAG_DELTA, dpcm,
};

// ─── public API ──────────────────────────────────────────────────────────────

/// Pack 8-bit-per-channel RGB into a 16-bit RGB555 little-endian word.
/// Layout: bit 15 = unused / format flag, 14..10 = R5, 9..5 = G5, 4..0 = B5.
#[inline]
pub const fn pack_rgb555(r: u8, g: u8, b: u8) -> u16 {
    (((r as u16) >> 3) << 10) | (((g as u16) >> 3) << 5) | ((b as u16) >> 3)
}

/// Encode a multi-frame RGB555 video.
///
/// Each entry in `frames` is a row-major `width * height` slice of u16
/// pixels in RGB555 (use [`pack_rgb555`] to build them from RGB888).
/// Width/height must be non-zero multiples of 8.
///
/// Lossless on every input where `pixel & 0x8000 == 0` (the format's
/// reserved bit). For high-detail content whose lossless raw form
/// would exceed MVE's 65 535-byte segment cap, see
/// [`encode_video_rgb555_lossy`] / [`encode_av_rgb555`].
pub fn encode_video_rgb555<W: Write>(
    width: u16,
    height: u16,
    frame_duration_us: u32,
    frames: &[&[u16]],
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    encode_av_rgb555(
        width,
        height,
        frame_duration_us,
        frames,
        false,
        None,
        name,
        out,
    )
}

/// Variant of [`encode_video_rgb555`] that opts in to a lossy 2×2
/// downsample fallback: blocks that would otherwise be emitted as
/// `0xb` raw (128 bytes) are instead lossily emitted as `0xc` 4×4
/// fill (32 bytes), keeping the top-left pixel of each 2×2 sub-block.
/// Use this when a fully-lossless raw frame would exceed MVE's
/// 65 535-byte segment cap (e.g. random noise at 640×480).
pub fn encode_video_rgb555_lossy<W: Write>(
    width: u16,
    height: u16,
    frame_duration_us: u32,
    frames: &[&[u16]],
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    encode_av_rgb555(
        width,
        height,
        frame_duration_us,
        frames,
        true,
        None,
        name,
        out,
    )
}

/// Like [`encode_video_rgb555`] but with optional per-frame audio
/// **and** an explicit `lossy_downsample` toggle. Set
/// `lossy_downsample = true` to engage the 2×2 downsample fallback
/// described in [`encode_video_rgb555_lossy`]; set it to `false` to
/// stay strictly lossless (and risk hitting the segment-cap on
/// extreme-detail input).
#[allow(clippy::too_many_arguments)]
pub fn encode_av_rgb555<W: Write>(
    width: u16,
    height: u16,
    frame_duration_us: u32,
    frames: &[&[u16]],
    lossy_downsample: bool,
    audio: Option<(&AudioOptions, &[Vec<i16>])>,
    name: impl Into<String>,
    out: &mut W,
) -> Result<()> {
    let _name = name.into();
    if width == 0 || height == 0 || !width.is_multiple_of(8) || !height.is_multiple_of(8) {
        return Err(MveEncodeError::InvalidDimensions { width, height });
    }
    if frame_duration_us == 0 {
        return Err(MveEncodeError::InvalidFrameDuration(frame_duration_us));
    }
    if frames.is_empty() {
        return Err(MveEncodeError::NoFrames);
    }
    let expected = width as usize * height as usize;
    for f in frames.iter() {
        if f.len() != expected {
            return Err(MveEncodeError::PixelBufferSize {
                got: f.len(),
                expected,
            });
        }
    }
    if let Some((aopts, per_frame)) = audio {
        if aopts.sample_rate > u16::MAX as u32 {
            return Err(MveEncodeError::AudioSampleRateTooHigh(aopts.sample_rate));
        }
        if aopts.channels != 1 && aopts.channels != 2 {
            return Err(MveEncodeError::AudioChannelsInvalid(aopts.channels));
        }
        if per_frame.len() != frames.len() {
            return Err(MveEncodeError::AudioFramesMismatch {
                got: per_frame.len(),
                expected: frames.len(),
            });
        }
    }

    out.write_all(SIGNATURE_24)?;
    out.write_all(&SIGNATURE_TAIL)?;

    let init_video = build_init_video_chunk_rgb555(width, height, frame_duration_us)?;
    write_chunk(out, CHUNK_INIT_VIDEO, &init_video)?;

    let init_audio = match audio {
        Some((aopts, _)) => build_init_audio_chunk_rgb555(aopts)?,
        None => Vec::new(),
    };
    write_chunk(out, CHUNK_INIT_AUDIO, &init_audio)?;

    let bw = (width as usize) >> 3;
    let bh = (height as usize) >> 3;
    let stride = width as usize;
    for (frame_idx, frame_pixels) in frames.iter().enumerate() {
        let prev = if frame_idx == 0 {
            None
        } else {
            Some(frames[frame_idx - 1])
        };
        let frame_audio =
            audio.map(|(aopts, per_frame)| (*aopts, per_frame[frame_idx].as_slice()));
        let body = build_frame_chunk_rgb555(
            frame_pixels,
            prev,
            stride,
            bw,
            bh,
            frame_idx > 0,
            lossy_downsample,
            frame_audio,
            frame_idx as u16,
        )?;
        write_chunk(out, CHUNK_FRAME, &body)?;
    }

    let mut end = Vec::with_capacity(4);
    write_segment(&mut end, OC_END_OF_STREAM, 0, &[]);
    write_chunk(out, CHUNK_END, &end)?;
    Ok(())
}

// ─── init chunks ─────────────────────────────────────────────────────────────

fn build_init_video_chunk_rgb555(
    width: u16,
    height: u16,
    frame_duration_us: u32,
) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    let mut timer = Vec::with_capacity(6);
    timer.extend_from_slice(&frame_duration_us.to_le_bytes());
    timer.extend_from_slice(&1u16.to_le_bytes());
    write_segment(&mut buf, OC_CREATE_TIMER, 0, &timer);

    // OC_VIDEO_MODE — HiColor variant. mcomp.exe with `HiColor` cfg
    // emits w=640, h=480, flags=0x0110; PS:T `cannon.mve` (a real
    // shipped HiColor cutscene) matches exactly.
    let mut mode = Vec::with_capacity(6);
    mode.extend_from_slice(&640u16.to_le_bytes());
    mode.extend_from_slice(&480u16.to_le_bytes());
    mode.extend_from_slice(&0x0110u16.to_le_bytes());
    write_segment(&mut buf, OC_VIDEO_MODE, 0, &mode);

    // OC_VIDEO_BUFFERS v2 — frame size in 8×8 blocks, format_flag=1
    // selects the 16 bpp / RGB555 decoder path.
    let mut buffers = Vec::with_capacity(8);
    buffers.extend_from_slice(&(width / 8).to_le_bytes());
    buffers.extend_from_slice(&(height / 8).to_le_bytes());
    buffers.extend_from_slice(&1u16.to_le_bytes()); // buf_count
    buffers.extend_from_slice(&1u16.to_le_bytes()); // format_flag = 1 → RGB555
    write_segment(&mut buf, OC_VIDEO_BUFFERS, 2, &buffers);

    write_segment(&mut buf, OC_END_OF_CHUNK, 0, &[]);

    if buf.len() > u16::MAX as usize {
        return Err(MveEncodeError::ChunkTooBig);
    }
    Ok(buf)
}

fn build_init_audio_chunk_rgb555(audio: &AudioOptions) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut payload = Vec::with_capacity(10);
    payload.extend_from_slice(&0u16.to_le_bytes());
    let mut flags: u16 = AUDIO_FLAG_16BIT | AUDIO_FLAG_COMPRESSED;
    if audio.channels == 2 {
        flags |= AUDIO_FLAG_STEREO;
    }
    payload.extend_from_slice(&flags.to_le_bytes());
    payload.extend_from_slice(&(audio.sample_rate as u16).to_le_bytes());
    payload.extend_from_slice(&0x0001_0000u32.to_le_bytes());
    write_segment(&mut buf, OC_AUDIO_BUFFERS, 1, &payload);
    write_segment(&mut buf, OC_END_OF_CHUNK, 0, &[]);
    if buf.len() > u16::MAX as usize {
        return Err(MveEncodeError::ChunkTooBig);
    }
    Ok(buf)
}

// ─── frame chunk ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_frame_chunk_rgb555(
    curr: &[u16],
    prev: Option<&[u16]>,
    stride: usize,
    bw: usize,
    bh: usize,
    use_delta: bool,
    lossy_downsample: bool,
    audio: Option<(AudioOptions, &[i16])>,
    seq: u16,
) -> Result<Vec<u8>> {
    let n_blocks = bw * bh;
    let mut opcodes = Vec::with_capacity(n_blocks);
    let mut color_stream = Vec::new();
    let mut motion_stream = Vec::new();

    for by in 0..bh {
        for bx in 0..bw {
            let block = read_block16(curr, stride, bx, by);
            let outcome = encode_block_rgb555(&block, prev, stride, bx, by, lossy_downsample);
            opcodes.push(outcome.opcode);
            color_stream.extend_from_slice(&outcome.color_bytes);
            motion_stream.extend_from_slice(&outcome.motion_bytes);
        }
    }

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

    // OC_VIDEO_DATA payload: 12 zero bytes header | u16 flags |
    // u16 motion_offset | colour stream | motion stream
    let frame_data_len = 2 + color_stream.len() + motion_stream.len();
    let motion_offset = (2 + color_stream.len()) as u16;
    let mut video = Vec::with_capacity(14 + frame_data_len);
    video.extend_from_slice(&[0u8; 12]);
    let flags: u16 = if use_delta { VIDEO_FLAG_DELTA } else { 0 };
    video.extend_from_slice(&flags.to_le_bytes());
    video.extend_from_slice(&motion_offset.to_le_bytes());
    video.extend_from_slice(&color_stream);
    video.extend_from_slice(&motion_stream);
    write_segment(&mut buf, OC_VIDEO_DATA, 0, &video);

    write_segment(&mut buf, OC_PLAY_VIDEO, 0, &[]);
    write_segment(&mut buf, OC_END_OF_CHUNK, 0, &[]);

    if buf.len() > u16::MAX as usize {
        return Err(MveEncodeError::ChunkTooBig);
    }
    Ok(buf)
}

// ─── per-block analysis & encoding ──────────────────────────────────────────

type Block16 = [[u16; 8]; 8];

fn read_block16(image: &[u16], stride: usize, bx: usize, by: usize) -> Block16 {
    let mut g = [[0u16; 8]; 8];
    let top_left = by * 8 * stride + bx * 8;
    for y in 0..8 {
        for x in 0..8 {
            g[y][x] = image[top_left + y * stride + x];
        }
    }
    g
}

struct BlockOutcome {
    opcode: u8,
    color_bytes: Vec<u8>,
    motion_bytes: Vec<u8>,
}

const OPC_SKIP: u8 = 0x0;
const OPC_MOTION: u8 = 0x4;
const OPC_MOTION_EXT: u8 = 0x5;
const OPC_2COLOR: u8 = 0x7;
const OPC_HALF_2COL: u8 = 0x8;
const OPC_QUAD_COLOR: u8 = 0x9;
const OPC_HALF_4COL: u8 = 0xa;
const OPC_RAW: u8 = 0xb;
const OPC_4X4_FILL: u8 = 0xc;
const OPC_QUADRANTS: u8 = 0xd;
const OPC_SOLID: u8 = 0xe;

fn encode_block_rgb555(
    curr: &Block16,
    prev_full: Option<&[u16]>,
    stride: usize,
    bx: usize,
    by: usize,
    lossy_downsample: bool,
) -> BlockOutcome {
    let color_only = |opcode: u8, bytes: Vec<u8>| BlockOutcome {
        opcode,
        color_bytes: bytes,
        motion_bytes: Vec::new(),
    };

    // 1. Skip — block identical to same position in previous frame.
    if let Some(prev) = prev_full {
        let p = read_block16(prev, stride, bx, by);
        if &p == curr {
            return BlockOutcome {
                opcode: OPC_SKIP,
                color_bytes: Vec::new(),
                motion_bytes: Vec::new(),
            };
        }
    }

    // 2. Solid — every pixel the same.
    let p0 = curr[0][0];
    if curr.iter().all(|row| row.iter().all(|&v| v == p0)) {
        return color_only(OPC_SOLID, p0.to_le_bytes().to_vec());
    }

    // 3. Motion compensation — exact 8×8 match within ±8 px in prev frame.
    if let Some(prev) = prev_full
        && let Some((dx, dy)) = find_motion_match16(curr, prev, stride, bx, by) {
            let b = (((dy + 8) as u8) << 4) | ((dx + 8) as u8);
            return BlockOutcome {
                opcode: OPC_MOTION,
                color_bytes: Vec::new(),
                motion_bytes: vec![b],
            };
        }

    // 3a. Extended motion compensation (`0x5`, 2 bytes). Search the
    //     full ±128 px window when `0x4` (±8) fails. Note: in the
    //     16-bit dispatch table, opcode `0x5` reads from the COLOUR
    //     sub-stream (`rc`), not the motion sub-stream (`rd`) — see
    //     `decode_frame16` in the decoder. So the two payload bytes
    //     go into `color_bytes` here.
    if let Some(prev) = prev_full
        && let Some((dx, dy)) = find_motion_match16_extended(curr, prev, stride, bx, by) {
            return BlockOutcome {
                opcode: OPC_MOTION_EXT,
                color_bytes: vec![dx as u8, dy as u8],
                motion_bytes: Vec::new(),
            };
        }

    let bit15_safe = !block_has_bit15(curr);
    let distinct = collect_distinct_block(curr, 5);

    // 4. 2-colour modes — only if every pixel has bit 15 = 0.
    if bit15_safe
        && let Some(d) = distinct.as_deref()
            && d.len() <= 2
                && let Some(bytes) = build_0x7_per_2x2(curr) {
                    return color_only(OPC_2COLOR, bytes);
                }

    // 5. Quadrants — 4 uniform 4×4 quadrants, any colour count.
    if let Some(quads) = try_quadrants(curr) {
        let mut bytes = Vec::with_capacity(8);
        for v in quads {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        return color_only(OPC_QUADRANTS, bytes);
    }

    // 6/7. 2-colour per-row, 3-4 colour per-2×2.
    if bit15_safe
        && let Some(d) = distinct.as_deref() {
            if d.len() <= 2
                && let Some(bytes) = build_0x7_per_row(curr) {
                    return color_only(OPC_2COLOR, bytes);
                }
            if (d.len() == 3 || d.len() == 4) && is_2x2_uniform(curr) {
                let bytes = build_0x9_per_2x2(curr, d);
                return color_only(OPC_QUAD_COLOR, bytes);
            }
        }

    // 8. 3-4 colour per-2×1 / per-1×2 (16 B); 9. 0x8 half-split (16 B).
    if bit15_safe {
        if let Some(d) = distinct.as_deref()
            && (d.len() == 3 || d.len() == 4) {
                if is_2x1_uniform(curr) {
                    let bytes = build_0x9_per_2x1_wide(curr, d);
                    return color_only(OPC_QUAD_COLOR, bytes);
                }
                if is_1x2_uniform(curr) {
                    let bytes = build_0x9_per_1x2_tall(curr, d);
                    return color_only(OPC_QUAD_COLOR, bytes);
                }
            }
        if let Some(bytes) = build_0x8_vertical_halves(curr) {
            return color_only(OPC_HALF_2COL, bytes);
        }
        if let Some(bytes) = build_0x8_horizontal_halves(curr) {
            return color_only(OPC_HALF_2COL, bytes);
        }
    }

    // 10/11. 0x9 per-pixel (24 B); 0x8 per-quadrant (24 B).
    if bit15_safe {
        if let Some(d) = distinct.as_deref()
            && (d.len() == 3 || d.len() == 4) {
                let bytes = build_0x9_per_pixel(curr, d);
                return color_only(OPC_QUAD_COLOR, bytes);
            }
        if let Some(bytes) = build_0x8_per_quadrant(curr) {
            return color_only(OPC_HALF_2COL, bytes);
        }
    }

    // 12. 0xa half-split (32 B).
    if bit15_safe {
        if let Some(bytes) = build_0xa_vertical_halves(curr) {
            return color_only(OPC_HALF_4COL, bytes);
        }
        if let Some(bytes) = build_0xa_horizontal_halves(curr) {
            return color_only(OPC_HALF_4COL, bytes);
        }
    }

    // 13. 4×4 fill — any colour count, every 2×2 uniform.
    if let Some(values) = try_4x4_fill(curr) {
        let mut bytes = Vec::with_capacity(32);
        for v in values {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        return color_only(OPC_4X4_FILL, bytes);
    }

    // 14. 0xa per-quadrant (48 B).
    if bit15_safe
        && let Some(bytes) = build_0xa_per_quadrant(curr) {
            return color_only(OPC_HALF_4COL, bytes);
        }

    // 14a. Lossy fallback — emit 0xc with 2×2 downsample (32 bytes)
    //      instead of 0xb raw (128 bytes). Top-left of each 2×2 wins.
    //      Only used when the caller opted in (`lossy_downsample`).
    if lossy_downsample {
        return color_only(OPC_4X4_FILL, build_4x4_fill_downsampled_rgb555(curr));
    }

    // 15. Raw 8×8 — strictly-lossless fallback (128 bytes).
    let mut bytes = Vec::with_capacity(128);
    for row in curr.iter() {
        for &v in row.iter() {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    color_only(OPC_RAW, bytes)
}

/// Lossy 2×2-downsample variant of the 0xc emitter: always succeeds
/// by taking the top-left pixel of each 2×2 sub-block as the
/// representative colour. Output is 16 × u16 LE = 32 bytes.
fn build_4x4_fill_downsampled_rgb555(curr: &Block16) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            out.extend_from_slice(&curr[y][x].to_le_bytes());
            x += 2;
        }
        y += 2;
    }
    out
}

// ─── helpers ────────────────────────────────────────────────────────────────

#[inline]
fn block_has_bit15(curr: &Block16) -> bool {
    curr.iter().any(|row| row.iter().any(|&v| v & 0x8000 != 0))
}

/// Distinct colours in encounter order, capped at `max`. Returns `None`
/// if more than `max` distinct values are seen.
fn collect_distinct_block(curr: &Block16, max: usize) -> Option<Vec<u16>> {
    let mut out: Vec<u16> = Vec::with_capacity(max + 1);
    for row in curr.iter() {
        for &v in row.iter() {
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

fn collect_distinct_region(
    curr: &Block16,
    y0: usize,
    x0: usize,
    y1: usize,
    x1: usize,
    max: usize,
) -> Option<Vec<u16>> {
    let mut out: Vec<u16> = Vec::with_capacity(max + 1);
    for y in y0..y1 {
        for x in x0..x1 {
            let v = curr[y][x];
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

fn try_quadrants(curr: &Block16) -> Option<[u16; 4]> {
    let q = |y0: usize, x0: usize| -> Option<u16> {
        let v = curr[y0][x0];
        for y in y0..y0 + 4 {
            for x in x0..x0 + 4 {
                if curr[y][x] != v {
                    return None;
                }
            }
        }
        Some(v)
    };
    Some([q(0, 0)?, q(0, 4)?, q(4, 0)?, q(4, 4)?])
}

fn try_4x4_fill(curr: &Block16) -> Option<[u16; 16]> {
    let mut out = [0u16; 16];
    let mut idx = 0;
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = curr[y][x];
            if curr[y][x + 1] != v || curr[y + 1][x] != v || curr[y + 1][x + 1] != v {
                return None;
            }
            out[idx] = v;
            idx += 1;
            x += 2;
        }
        y += 2;
    }
    Some(out)
}

fn is_2x2_uniform(curr: &Block16) -> bool {
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = curr[y][x];
            if curr[y][x + 1] != v || curr[y + 1][x] != v || curr[y + 1][x + 1] != v {
                return false;
            }
            x += 2;
        }
        y += 2;
    }
    true
}

fn is_2x1_uniform(curr: &Block16) -> bool {
    for y in 0..8 {
        let mut x = 0;
        while x < 8 {
            if curr[y][x] != curr[y][x + 1] {
                return false;
            }
            x += 2;
        }
    }
    true
}

fn is_1x2_uniform(curr: &Block16) -> bool {
    let mut y = 0;
    while y < 8 {
        for x in 0..8 {
            if curr[y][x] != curr[y + 1][x] {
                return false;
            }
        }
        y += 2;
    }
    true
}

/// Brute-force ±8 px motion search.
fn find_motion_match16(
    curr: &Block16,
    prev: &[u16],
    stride: usize,
    bx: usize,
    by: usize,
) -> Option<(i32, i32)> {
    let height = prev.len() / stride;
    let block_x = bx * 8;
    let block_y = by * 8;
    for dy in -8i32..=7 {
        let src_y = block_y as i32 + dy;
        if src_y < 0 || src_y + 8 > height as i32 {
            continue;
        }
        for dx in -8i32..=7 {
            let src_x = block_x as i32 + dx;
            if src_x < 0 || src_x + 8 > stride as i32 {
                continue;
            }
            let mut ok = true;
            'check: for y in 0..8 {
                let row_off = (src_y as usize + y) * stride + src_x as usize;
                for x in 0..8 {
                    if prev[row_off + x] != curr[y][x] {
                        ok = false;
                        break 'check;
                    }
                }
            }
            if ok {
                return Some((dx, dy));
            }
        }
    }
    None
}

/// Brute-force ±128 px motion search, **excluding** the inner ±8 px
/// region already covered by [`find_motion_match16`]. Returned offsets
/// are guaranteed signed-i8 representable.
fn find_motion_match16_extended(
    curr: &Block16,
    prev: &[u16],
    stride: usize,
    bx: usize,
    by: usize,
) -> Option<(i8, i8)> {
    let height = prev.len() / stride;
    let block_x = bx * 8;
    let block_y = by * 8;
    for dy in -128i32..=127 {
        let src_y = block_y as i32 + dy;
        if src_y < 0 || src_y + 8 > height as i32 {
            continue;
        }
        for dx in -128i32..=127 {
            // Skip the ±8 inner window — already swept by `0x4`'s
            // search and would cost an extra byte if we re-emitted.
            if (-8..=7).contains(&dx) && (-8..=7).contains(&dy) {
                continue;
            }
            let src_x = block_x as i32 + dx;
            if src_x < 0 || src_x + 8 > stride as i32 {
                continue;
            }
            let mut ok = true;
            'check: for y in 0..8 {
                let row_off = (src_y as usize + y) * stride + src_x as usize;
                for x in 0..8 {
                    if prev[row_off + x] != curr[y][x] {
                        ok = false;
                        break 'check;
                    }
                }
            }
            if ok {
                return Some((dx as i8, dy as i8));
            }
        }
    }
    None
}

// ─── 0x7 (2-colour) ─────────────────────────────────────────────────────────
//
// Decoder reads p0 (u16), p1 (u16) then branches on bit 15 of p0:
// - bit 15 SET   → per-2×2 sub-mode, 2 more bytes of mask (16 sub-blocks)
// - bit 15 CLEAR → per-row sub-mode, 8 more bytes of mask (1 byte per row)
//
// The decoder strips bit 15 from p0 in the per-2×2 branch only. For
// per-row, bit 15 stays whatever the encoder wrote (must be 0 for the
// branch to fire). Encoder picks p0 = "the colour we want index 0 to
// mean" (bit clear or set as required); p1 = the other colour.

fn build_0x7_per_2x2(curr: &Block16) -> Option<Vec<u8>> {
    let distinct = collect_distinct_block(curr, 2)?;
    if distinct.is_empty() {
        return None;
    }
    let p0 = distinct[0];
    let p1 = if distinct.len() == 2 { distinct[1] } else { p0 };

    // Every 2×2 must be uniform AND made of {p0, p1}.
    let mut mask = 0u16;
    let mut bit = 1u16;
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = curr[y][x];
            if curr[y][x + 1] != v || curr[y + 1][x] != v || curr[y + 1][x + 1] != v {
                return None;
            }
            if v == p1 {
                mask |= bit;
            } else if v != p0 {
                return None;
            }
            bit <<= 1;
            x += 2;
        }
        y += 2;
    }

    let mut out = Vec::with_capacity(6);
    out.extend_from_slice(&(p0 | 0x8000).to_le_bytes()); // bit 15 set → per-2×2
    out.extend_from_slice(&p1.to_le_bytes());
    out.extend_from_slice(&mask.to_le_bytes());
    Some(out)
}

fn build_0x7_per_row(curr: &Block16) -> Option<Vec<u8>> {
    let distinct = collect_distinct_block(curr, 2)?;
    if distinct.is_empty() {
        return None;
    }
    let p0 = distinct[0];
    let p1 = if distinct.len() == 2 { distinct[1] } else { p0 };

    let mut rows = [0u8; 8];
    for y in 0..8 {
        for x in 0..8 {
            let v = curr[y][x];
            if v == p1 {
                rows[y] |= 1 << x;
            } else if v != p0 {
                return None;
            }
        }
    }

    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(&p0.to_le_bytes()); // bit 15 already 0 (caller checked)
    out.extend_from_slice(&p1.to_le_bytes());
    out.extend_from_slice(&rows);
    Some(out)
}

// ─── 0x9 (3-4 colour) ───────────────────────────────────────────────────────
//
// Decoder reads p[0..4] (4 × u16) then branches on bit 15 of p[0] and p[2]:
// - p[0] clear + p[2] clear → per-pixel  (16 bytes mask)
// - p[0] clear + p[2] set   → per-2×2   (4 bytes mask)
// - p[0] set   + p[2] clear → per-2×1   (8 bytes mask)
// - p[0] set   + p[2] set   → per-1×2   (8 bytes mask)
//
// In each branch the decoder strips bit 15 from whichever palette
// positions are tagged. The encoder writes the palette ordering so
// that each pixel can be reconstructed from a 2-bit index into p[0..4].

#[inline]
fn pad_to_4(distinct: &[u16]) -> [u16; 4] {
    match distinct.len() {
        3 => [distinct[0], distinct[1], distinct[2], distinct[2]],
        4 => [distinct[0], distinct[1], distinct[2], distinct[3]],
        _ => unreachable!("collect_distinct_block must have returned 3..=4 elements"),
    }
}

#[inline]
fn idx_in(p: &[u16; 4], v: u16) -> u32 {
    p.iter().position(|&c| c == v).expect("colour must be in palette") as u32
}

fn build_0x9_per_2x2(curr: &Block16, distinct: &[u16]) -> Vec<u8> {
    let p = pad_to_4(distinct);
    let mut out = Vec::with_capacity(12);
    // p[0] clear, p[2] set → per-2×2 sub-mode
    out.extend_from_slice(&p[0].to_le_bytes());
    out.extend_from_slice(&p[1].to_le_bytes());
    out.extend_from_slice(&(p[2] | 0x8000).to_le_bytes());
    out.extend_from_slice(&p[3].to_le_bytes());
    let mut flags: u32 = 0;
    let mut shifter = 0;
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            flags |= idx_in(&p, curr[y][x]) << shifter;
            shifter += 2;
            x += 2;
        }
        y += 2;
    }
    out.extend_from_slice(&flags.to_le_bytes());
    out
}

fn build_0x9_per_2x1_wide(curr: &Block16, distinct: &[u16]) -> Vec<u8> {
    let p = pad_to_4(distinct);
    let mut out = Vec::with_capacity(16);
    // p[0] set, p[2] clear → per-2×1 sub-mode
    out.extend_from_slice(&(p[0] | 0x8000).to_le_bytes());
    out.extend_from_slice(&p[1].to_le_bytes());
    out.extend_from_slice(&p[2].to_le_bytes());
    out.extend_from_slice(&p[3].to_le_bytes());
    let mut y = 0;
    while y < 8 {
        let mut flags: u32 = 0;
        let mut shifter = 0;
        for dy in 0..4usize {
            let mut x = 0;
            while x < 8 {
                flags |= idx_in(&p, curr[y + dy][x]) << shifter;
                shifter += 2;
                x += 2;
            }
        }
        out.extend_from_slice(&flags.to_le_bytes());
        y += 4;
    }
    out
}

fn build_0x9_per_1x2_tall(curr: &Block16, distinct: &[u16]) -> Vec<u8> {
    let p = pad_to_4(distinct);
    let mut out = Vec::with_capacity(16);
    // p[0] set, p[2] set → per-1×2 sub-mode
    out.extend_from_slice(&(p[0] | 0x8000).to_le_bytes());
    out.extend_from_slice(&p[1].to_le_bytes());
    out.extend_from_slice(&(p[2] | 0x8000).to_le_bytes());
    out.extend_from_slice(&p[3].to_le_bytes());
    let mut y = 0;
    while y < 8 {
        let mut flags: u32 = 0;
        let mut shifter = 0;
        let mut dy = 0;
        while dy < 4 {
            for x in 0..8 {
                flags |= idx_in(&p, curr[y + dy][x]) << shifter;
                shifter += 2;
            }
            dy += 2;
        }
        out.extend_from_slice(&flags.to_le_bytes());
        y += 4;
    }
    out
}

fn build_0x9_per_pixel(curr: &Block16, distinct: &[u16]) -> Vec<u8> {
    let p = pad_to_4(distinct);
    let mut out = Vec::with_capacity(24);
    // p[0] clear, p[2] clear → per-pixel sub-mode
    for &v in p.iter() {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for y in 0..8 {
        let mut flags: u16 = 0;
        for x in 0..8 {
            let idx = idx_in(&p, curr[y][x]) as u16;
            flags |= idx << (x * 2);
        }
        out.extend_from_slice(&flags.to_le_bytes());
    }
    out
}

// ─── 0x8 (2-colour per partition) ───────────────────────────────────────────
//
// Per-quadrant (24 B): 4 × (p_a u16, p_b u16, b_lo u8, b_hi u8). Bit 15
// of the very first u16 (p[0]) must be CLEAR.
// Half-split (16 B): p[0] u16, p[1] u16, b[0..4], p[2] u16, p[3] u16,
// b[4..8]. Bit 15 of p[0] SET; bit 15 of p[2] picks vertical (CLEAR) or
// horizontal (SET).
//
// The 16-bit encoder doesn't need the 8-bit "palette ordering" hacks
// (`pp0 > pp1` to force a sub-mode) — bit 15 is the explicit flag.
// When a half/quadrant has only one colour, we reuse it as both
// palette entries — the mask bits then don't matter (both lookups
// resolve to the same colour).

fn build_0x8_per_quadrant(curr: &Block16) -> Option<Vec<u8>> {
    // Each 4×4 quadrant must have ≤ 2 distinct colours.
    let quads = [
        ((0usize, 0usize), 0usize), // TL → slots 0,1
        ((4, 0), 2),                 // BL → slots 2,3
        ((0, 4), 4),                 // TR → slots 4,5
        ((4, 4), 6),                 // BR → slots 6,7
    ];
    let mut p = [0u16; 8];
    let mut b = [0u8; 8];
    for &((qy, qx), slot) in quads.iter() {
        let colours = collect_distinct_region(curr, qy, qx, qy + 4, qx + 4, 2)?;
        let pp0 = colours[0];
        let pp1 = if colours.len() == 2 { colours[1] } else { pp0 };
        p[slot] = pp0;
        p[slot + 1] = pp1;
        let (lo, hi) = quadrant_mask_2col(curr, qy, qx, pp0, pp1);
        b[slot] = lo;
        b[slot + 1] = hi;
    }
    let mut out = Vec::with_capacity(24);
    // First u16 must have bit 15 CLEAR to select per-quadrant.
    out.extend_from_slice(&p[0].to_le_bytes());
    out.extend_from_slice(&p[1].to_le_bytes());
    out.push(b[0]);
    out.push(b[1]);
    for &slot in &[2usize, 4, 6] {
        out.extend_from_slice(&p[slot].to_le_bytes());
        out.extend_from_slice(&p[slot + 1].to_le_bytes());
        out.push(b[slot]);
        out.push(b[slot + 1]);
    }
    Some(out)
}

/// Pack the 16 mask bits of one 4×4 quadrant into two bytes:
/// row 0 → low nibble of `lo`, row 1 → high nibble of `lo`,
/// row 2 → low nibble of `hi`, row 3 → high nibble of `hi`.
/// Bit value 1 = pixel matches `pp1`, 0 = matches `pp0`.
fn quadrant_mask_2col(curr: &Block16, qy: usize, qx: usize, pp0: u16, pp1: u16) -> (u8, u8) {
    let mut lo = 0u8;
    let mut hi = 0u8;
    for dy in 0..4 {
        for dx in 0..4 {
            let v = curr[qy + dy][qx + dx];
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

fn build_0x8_vertical_halves(curr: &Block16) -> Option<Vec<u8>> {
    // Left half = x in 0..4, right half = x in 4..8.
    let left = collect_distinct_region(curr, 0, 0, 8, 4, 2)?;
    let right = collect_distinct_region(curr, 0, 4, 8, 8, 2)?;
    let p0 = left[0];
    let p1 = if left.len() == 2 { left[1] } else { p0 };
    let p2 = right[0];
    let p3 = if right.len() == 2 { right[1] } else { p2 };
    let mut b = [0u8; 8];
    write_vertical_halves_mask(curr, &mut b, p0, p1, p2, p3);
    let mut out = Vec::with_capacity(16);
    // p[0] | 0x8000 → half-split branch; p[2] bit 15 CLEAR → vertical.
    out.extend_from_slice(&(p0 | 0x8000).to_le_bytes());
    out.extend_from_slice(&p1.to_le_bytes());
    out.push(b[0]);
    out.push(b[1]);
    out.push(b[2]);
    out.push(b[3]);
    out.extend_from_slice(&p2.to_le_bytes());
    out.extend_from_slice(&p3.to_le_bytes());
    out.push(b[4]);
    out.push(b[5]);
    out.push(b[6]);
    out.push(b[7]);
    Some(out)
}

fn build_0x8_horizontal_halves(curr: &Block16) -> Option<Vec<u8>> {
    let top = collect_distinct_region(curr, 0, 0, 4, 8, 2)?;
    let bot = collect_distinct_region(curr, 4, 0, 8, 8, 2)?;
    let p0 = top[0];
    let p1 = if top.len() == 2 { top[1] } else { p0 };
    let p2 = bot[0];
    let p3 = if bot.len() == 2 { bot[1] } else { p2 };
    let mut b = [0u8; 8];
    for y in 0..8 {
        let (pp0, pp1) = if y < 4 { (p0, p1) } else { (p2, p3) };
        let mut row = 0u8;
        for x in 0..8 {
            let v = curr[y][x];
            let bit: u8 = if v == pp1 {
                1
            } else if v == pp0 {
                0
            } else {
                return None; // shouldn't fire after collect_distinct_region
            };
            row |= bit << x;
        }
        b[y] = row;
    }
    let mut out = Vec::with_capacity(16);
    // p[0] | 0x8000 → half-split branch; p[2] | 0x8000 → horizontal.
    out.extend_from_slice(&(p0 | 0x8000).to_le_bytes());
    out.extend_from_slice(&p1.to_le_bytes());
    out.push(b[0]);
    out.push(b[1]);
    out.push(b[2]);
    out.push(b[3]);
    out.extend_from_slice(&(p2 | 0x8000).to_le_bytes());
    out.extend_from_slice(&p3.to_le_bytes());
    out.push(b[4]);
    out.push(b[5]);
    out.push(b[6]);
    out.push(b[7]);
    Some(out)
}

/// Build the 8 mask bytes for the 0x8 vertical-halves sub-mode. The
/// decoder reuses the `pack_flags_8` packing so each byte carries two
/// 4-pixel half-rows in low/high nibble; bytes 0-3 cover the left
/// half (x<4), bytes 4-7 cover the right half (x≥4).
fn write_vertical_halves_mask(
    curr: &Block16,
    b: &mut [u8; 8],
    p0: u16,
    p1: u16,
    p2: u16,
    p3: u16,
) {
    for y in 0..8 {
        for x in 0..8 {
            let v = curr[y][x];
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

// ─── 0xa (4-colour per partition) ───────────────────────────────────────────
//
// Per-quadrant (48 B): four chunks of (4 × u16 palette + 4 × u8 mask).
// Bit 15 of p[0] CLEAR.
// Half-split (32 B): p[0..4] u16, b[0..4] u8, b[4..8] u8, p[4..8] u16,
// b[8..16] u8. Bit 15 of p[0] SET; bit 15 of p[4] picks vertical
// (CLEAR) or horizontal (SET).

fn build_0xa_per_quadrant(curr: &Block16) -> Option<Vec<u8>> {
    let quads = [
        ((0usize, 0usize), 0usize),  // TL → palette slots 0..4, mask bytes 0..4
        ((4, 0), 4),                  // BL → 4..8, 4..8
        ((0, 4), 8),                  // TR → 8..12, 8..12
        ((4, 4), 12),                 // BR → 12..16, 12..16
    ];
    let mut p = [0u16; 16];
    let mut b = [0u8; 16];
    for &((qy, qx), slot) in quads.iter() {
        let colours = collect_distinct_region(curr, qy, qx, qy + 4, qx + 4, 4)?;
        let padded = pad_to_4_slice(&colours);
        p[slot..slot + 4].copy_from_slice(&padded);
        for dy in 0..4 {
            let mut row = 0u8;
            for dx in 0..4 {
                let v = curr[qy + dy][qx + dx];
                let idx = padded.iter().position(|&c| c == v).unwrap() as u8;
                row |= (idx & 0x03) << (dx * 2);
            }
            b[slot + dy] = row;
        }
    }
    // p[0] bit 15 must be CLEAR. Caller ensured no source pixel has
    // bit 15 set, so the colour at p[0] is already in [0, 0x7fff].
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(&p[0].to_le_bytes());
    out.extend_from_slice(&p[1].to_le_bytes());
    out.extend_from_slice(&p[2].to_le_bytes());
    out.extend_from_slice(&p[3].to_le_bytes());
    out.extend_from_slice(&b[0..4]);
    for &start in &[4usize, 8, 12] {
        for i in 0..4 {
            out.extend_from_slice(&p[start + i].to_le_bytes());
        }
        out.extend_from_slice(&b[start..start + 4]);
    }
    Some(out)
}

fn build_0xa_vertical_halves(curr: &Block16) -> Option<Vec<u8>> {
    let left = collect_distinct_region(curr, 0, 0, 8, 4, 4)?;
    let right = collect_distinct_region(curr, 0, 4, 8, 8, 4)?;
    let p_left = pad_to_4_slice(&left);
    let p_right = pad_to_4_slice(&right);
    let mut b = [0u8; 16];
    for y in 0..8 {
        let mut left_mask = 0u8;
        for x in 0..4 {
            let idx = p_left.iter().position(|&c| c == curr[y][x]).unwrap() as u8;
            left_mask |= (idx & 0x03) << (x * 2);
        }
        b[y] = left_mask;
        let mut right_mask = 0u8;
        for x in 4..8 {
            let idx = p_right.iter().position(|&c| c == curr[y][x]).unwrap() as u8;
            right_mask |= (idx & 0x03) << ((x - 4) * 2);
        }
        b[y + 8] = right_mask;
    }
    Some(emit_0xa_halves(&p_left, &p_right, &b, /*horizontal=*/ false))
}

fn build_0xa_horizontal_halves(curr: &Block16) -> Option<Vec<u8>> {
    let top = collect_distinct_region(curr, 0, 0, 4, 8, 4)?;
    let bot = collect_distinct_region(curr, 4, 0, 8, 8, 4)?;
    let p_top = pad_to_4_slice(&top);
    let p_bot = pad_to_4_slice(&bot);
    let mut b = [0u8; 16];
    for y in 0..8 {
        let pal = if y < 4 { &p_top } else { &p_bot };
        let mut left_mask = 0u8;
        for x in 0..4 {
            let idx = pal.iter().position(|&c| c == curr[y][x]).unwrap() as u8;
            left_mask |= (idx & 0x03) << (x * 2);
        }
        b[y * 2] = left_mask;
        let mut right_mask = 0u8;
        for x in 4..8 {
            let idx = pal.iter().position(|&c| c == curr[y][x]).unwrap() as u8;
            right_mask |= (idx & 0x03) << ((x - 4) * 2);
        }
        b[y * 2 + 1] = right_mask;
    }
    Some(emit_0xa_halves(&p_top, &p_bot, &b, /*horizontal=*/ true))
}

/// Emit a 32-byte 0xa half-split payload. The decoder reads the first
/// half's palette, then 8 mask bytes, then the second half's palette,
/// then 8 more mask bytes — see `decode16_0xa`'s "half-split" branch.
fn emit_0xa_halves(
    first_pal: &[u16; 4],
    second_pal: &[u16; 4],
    b: &[u8; 16],
    horizontal: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    // First palette entry: bit 15 SET → half-split branch.
    out.extend_from_slice(&(first_pal[0] | 0x8000).to_le_bytes());
    out.extend_from_slice(&first_pal[1].to_le_bytes());
    out.extend_from_slice(&first_pal[2].to_le_bytes());
    out.extend_from_slice(&first_pal[3].to_le_bytes());
    out.extend_from_slice(&b[0..4]);
    out.extend_from_slice(&b[4..8]);
    // p[4] bit 15: SET → horizontal, CLEAR → vertical.
    let p4 = if horizontal {
        second_pal[0] | 0x8000
    } else {
        second_pal[0]
    };
    out.extend_from_slice(&p4.to_le_bytes());
    out.extend_from_slice(&second_pal[1].to_le_bytes());
    out.extend_from_slice(&second_pal[2].to_le_bytes());
    out.extend_from_slice(&second_pal[3].to_le_bytes());
    out.extend_from_slice(&b[8..16]);
    out
}

fn pad_to_4_slice(colours: &[u16]) -> [u16; 4] {
    let mut s = [0u16; 4];
    let n = colours.len().min(4);
    s[..n].copy_from_slice(&colours[..n]);
    let last = colours[n - 1];
    for slot in s.iter_mut().skip(n) {
        *slot = last;
    }
    s
}

// ─── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_rgb555_round_trip() {
        assert_eq!(pack_rgb555(0xff, 0, 0), 0b011111_00000_00000);
        assert_eq!(pack_rgb555(0, 0xff, 0), 0b000000_11111_00000);
        assert_eq!(pack_rgb555(0, 0, 0xff), 0b000000_00000_11111);
        assert_eq!(pack_rgb555(0, 0, 0), 0);
    }

    #[test]
    fn quadrants_detected() {
        let mut blk = [[0u16; 8]; 8];
        for y in 0..4 {
            for x in 0..4 {
                blk[y][x] = 0x1234;
            }
        }
        for y in 0..4 {
            for x in 4..8 {
                blk[y][x] = 0x4321;
            }
        }
        for y in 4..8 {
            for x in 0..4 {
                blk[y][x] = 0x0aaa;
            }
        }
        for y in 4..8 {
            for x in 4..8 {
                blk[y][x] = 0x0001;
            }
        }
        let q = try_quadrants(&blk).unwrap();
        assert_eq!(q, [0x1234, 0x4321, 0x0aaa, 0x0001]);
    }
}
