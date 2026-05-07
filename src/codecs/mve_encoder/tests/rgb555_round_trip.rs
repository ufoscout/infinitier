//! End-to-end round-trips for the RGB555 (HiColor) encoder path:
//! synth → encode → decode → check the decoded pixels match the
//! source RGB555 values bit-exactly (after the standard 5→8 bit
//! replication the decoder applies).

use infinitier_datasource::DataSource;
use infinitier_mve_decoder::{MveDecoder, VideoFormat};
use infinitier_mve_encoder::{encode_video_rgb555, pack_rgb555};

/// Expand a single 5-bit channel to 8 bits using the canonical
/// replicated-bit trick `(x << 3) | (x >> 2)` — same as the decoder.
fn expand5(c5: u8) -> u8 {
    (c5 << 3) | (c5 >> 2)
}

/// Manually convert an RGB555 word to an RGBA8888 4-tuple matching
/// what the decoder produces.
fn rgb555_to_rgba(px: u16) -> [u8; 4] {
    let r = ((px >> 10) & 0x1f) as u8;
    let g = ((px >> 5) & 0x1f) as u8;
    let b = (px & 0x1f) as u8;
    [expand5(r), expand5(g), expand5(b), 0xff]
}

fn open_decoder(bytes: Vec<u8>) -> MveDecoder<Box<dyn infinitier_datasource::DataTrait>> {
    let ds = DataSource::new(bytes);
    MveDecoder::new(ds.reader().unwrap(), "rgb555-test").expect("open MVE")
}

fn decode_all(bytes: Vec<u8>) -> (u16, u16, VideoFormat, Vec<Vec<u8>>) {
    let mut dec = open_decoder(bytes);
    let (w, h, fmt) = (dec.width(), dec.height(), dec.format());
    let mut frames = Vec::new();
    while let Some(f) = dec.next_frame().unwrap() {
        frames.push(f.video.pixels);
    }
    (w, h, fmt, frames)
}

/// Decode a fresh MVE and assert each frame's RGBA pixels match the
/// expected RGBA derived from `frames` (a slice of u16 RGB555 frames).
fn assert_round_trip_bit_exact(width: u16, height: u16, frames: &[Vec<u16>]) {
    let frame_refs: Vec<&[u16]> = frames.iter().map(|f| f.as_slice()).collect();
    let mut buf = Vec::new();
    encode_video_rgb555(width, height, 66_667, &frame_refs, "rt", &mut buf).unwrap();

    let (w, h, fmt, decoded) = decode_all(buf);
    assert_eq!((w, h), (width, height), "width/height mismatch");
    assert_eq!(fmt, VideoFormat::Rgb555, "decoder must report Rgb555");
    assert_eq!(decoded.len(), frames.len(), "frame count mismatch");

    for (idx, (src_frame, dec_frame)) in frames.iter().zip(decoded.iter()).enumerate() {
        for (px_idx, &src_px) in src_frame.iter().enumerate() {
            let expected = rgb555_to_rgba(src_px);
            let got = &dec_frame[px_idx * 4..px_idx * 4 + 4];
            assert_eq!(
                got, &expected,
                "frame {idx} pixel {px_idx} mismatch: src=0x{src_px:04x} expected={expected:?} got={got:?}"
            );
        }
    }
}

// ─── single-frame mode coverage ─────────────────────────────────────────────

#[test]
fn solid_block_uses_0xe() {
    let blue = pack_rgb555(0, 0, 0xff);
    let frames = vec![vec![blue; 64]];
    assert_round_trip_bit_exact(8, 8, &frames);
}

#[test]
fn solid_full_frame_round_trip() {
    let red = pack_rgb555(0xff, 0, 0);
    let pixels = vec![red; 320 * 240];
    let frames = vec![pixels];
    assert_round_trip_bit_exact(320, 240, &frames);
}

#[test]
fn quadrants_block_round_trip() {
    let mut block = vec![0u16; 64];
    let tl = pack_rgb555(0xff, 0, 0);
    let tr = pack_rgb555(0, 0xff, 0);
    let bl = pack_rgb555(0, 0, 0xff);
    let br = pack_rgb555(0xff, 0xff, 0);
    for y in 0..4 {
        for x in 0..4 {
            block[y * 8 + x] = tl;
        }
    }
    for y in 0..4 {
        for x in 4..8 {
            block[y * 8 + x] = tr;
        }
    }
    for y in 4..8 {
        for x in 0..4 {
            block[y * 8 + x] = bl;
        }
    }
    for y in 4..8 {
        for x in 4..8 {
            block[y * 8 + x] = br;
        }
    }
    assert_round_trip_bit_exact(8, 8, &[block]);
}

#[test]
fn fill_4x4_round_trip() {
    // Each 2×2 sub-block is uniform; 16 sub-blocks → 16 distinct
    // values to force the 0xc path (per_quadrant fails).
    let mut block = vec![0u16; 64];
    let mut v = 1u16;
    let mut y = 0usize;
    while y < 8 {
        let mut x = 0usize;
        while x < 8 {
            for dy in 0..2 {
                for dx in 0..2 {
                    block[(y + dy) * 8 + x + dx] = v;
                }
            }
            v += 1;
            x += 2;
        }
        y += 2;
    }
    assert_round_trip_bit_exact(8, 8, &[block]);
}

#[test]
fn raw_block_round_trip() {
    // 64 distinct RGB555 values → forces 0xb (raw, 128 bytes).
    let mut block = vec![0u16; 64];
    for i in 0..64u16 {
        // Spread evenly across the 5-bit B channel and the 5-bit R
        // channel so every pixel is unique.
        let r = (i & 0x1f) as u8;
        let b = ((i >> 5) & 0x07) as u8;
        block[i as usize] = (((r as u16) << 10) | (b as u16)) & 0x7fff;
    }
    assert_round_trip_bit_exact(8, 8, &[block]);
}

// ─── multi-frame: skip + motion ─────────────────────────────────────────────

#[test]
fn unchanged_frame_uses_skip() {
    let red = pack_rgb555(0xff, 0, 0);
    let f0: Vec<u16> = vec![red; 64];
    let f1 = f0.clone();
    assert_round_trip_bit_exact(8, 8, &[f0, f1]);
}

#[test]
fn horizontal_shift_round_trip() {
    // 16×8 frame, two solid blocks of opposite colours that swap
    // between frames. Both frames have only solid blocks so this
    // ends up using mode 0xe twice — useful as a basic skip+swap
    // sanity check, complemented by `non_uniform_pan_uses_motion`.
    let red = pack_rgb555(0xff, 0, 0);
    let blue = pack_rgb555(0, 0, 0xff);
    let mut f0 = vec![blue; 16 * 8];
    for y in 0..8 {
        for x in 0..8 {
            f0[y * 16 + x] = red;
        }
    }
    let mut f1 = vec![blue; 16 * 8];
    for y in 0..8 {
        for x in 8..16 {
            f1[y * 16 + x] = red;
        }
    }
    assert_round_trip_bit_exact(16, 8, &[f0, f1]);
}

#[test]
fn non_uniform_pan_uses_motion() {
    // 16×8 frame. Build a non-uniform 8×8 pattern, place it at the
    // left in frame 0 and at the right in frame 1. Block 1 of frame 1
    // must find an exact match at (dx=-8, dy=0) in frame 0 → mode 0x4.
    use infinitier_datasource::DataSource;
    use infinitier_mve_decoder::MveDecoder;

    let blue = pack_rgb555(0, 0, 0xff);
    let mut pattern = [0u16; 64];
    for (i, slot) in pattern.iter_mut().enumerate() {
        *slot = pack_rgb555(
            (i * 4) as u8,
            ((i * 7) & 0xff) as u8,
            ((i * 3) & 0xff) as u8,
        );
    }
    let mut f0 = vec![blue; 16 * 8];
    let mut f1 = vec![blue; 16 * 8];
    for y in 0..8 {
        for x in 0..8 {
            f0[y * 16 + x] = pattern[y * 8 + x];
        }
    }
    for y in 0..8 {
        for x in 8..16 {
            f1[y * 16 + x] = pattern[y * 8 + (x - 8)];
        }
    }

    let frame_refs: Vec<&[u16]> = vec![&f0, &f1];
    let mut buf = Vec::new();
    encode_video_rgb555(16, 8, 66_667, &frame_refs, "pan", &mut buf).unwrap();

    let ds = DataSource::new(buf);
    let mut dec: MveDecoder<Box<dyn infinitier_datasource::DataTrait>> =
        MveDecoder::new(ds.reader().unwrap(), "pan").unwrap();
    while dec.next_frame().unwrap().is_some() {}
    let stats = dec.block_mode_stats();

    assert!(
        stats.video16[0x4] >= 1,
        "expected at least one 0x4 motion block, got: {:?}",
        stats.video16
    );

    // And full round-trip correctness.
    assert_round_trip_bit_exact(16, 8, &[f0, f1]);
}

// ─── header / format-flag check ─────────────────────────────────────────────

#[test]
fn init_chunk_emits_hicolor_signals() {
    let pixels = vec![0u16; 64];
    let mut buf = Vec::new();
    encode_video_rgb555(8, 8, 66_667, &[&pixels], "hc", &mut buf).unwrap();

    // Walk segments to find OC_VIDEO_MODE (0x0a) and OC_VIDEO_BUFFERS (0x05).
    const SIG_LEN: usize = 26;
    let mut off = SIG_LEN;
    let mut video_mode: Option<Vec<u8>> = None;
    let mut video_buffers: Option<(u8, Vec<u8>)> = None;
    let mut palette_seen = false;
    'outer: while off + 4 <= buf.len() {
        let chunk_size = u16::from_le_bytes([buf[off], buf[off + 1]]) as usize;
        off += 4;
        let end = off + chunk_size;
        let mut p = off;
        while p + 4 <= end {
            let seg_size = u16::from_le_bytes([buf[p], buf[p + 1]]) as usize;
            let opcode = buf[p + 2];
            let version = buf[p + 3];
            let payload = buf[p + 4..p + 4 + seg_size].to_vec();
            match opcode {
                0x0a => video_mode = Some(payload),
                0x05 => video_buffers = Some((version, payload)),
                0x0c => palette_seen = true,
                _ => {}
            }
            if video_mode.is_some() && video_buffers.is_some() {
                break 'outer;
            }
            p += 4 + seg_size;
        }
        off = end;
    }

    let vm = video_mode.expect("OC_VIDEO_MODE missing");
    assert_eq!(
        vm,
        vec![0x80, 0x02, 0xE0, 0x01, 0x10, 0x01],
        "OC_VIDEO_MODE must match what mcomp HiColor emits (640×480, flags=0x0110); got {vm:02x?}"
    );

    let (vb_version, vb) = video_buffers.expect("OC_VIDEO_BUFFERS missing");
    assert!(
        vb_version > 1,
        "OC_VIDEO_BUFFERS must be v2+ for the format_flag to be honoured by the decoder"
    );
    let format_flag = u16::from_le_bytes([vb[6], vb[7]]);
    assert_eq!(format_flag, 1, "format_flag must be 1 to select RGB555");

    assert!(!palette_seen, "OC_PALETTE must be absent in HiColor output");
}

// ─── helpers for mode-specific tests ────────────────────────────────────────

fn block_mode_stats_for(width: u16, height: u16, frames: &[Vec<u16>]) -> [u64; 16] {
    let frame_refs: Vec<&[u16]> = frames.iter().map(|f| f.as_slice()).collect();
    let mut buf = Vec::new();
    encode_video_rgb555(width, height, 66_667, &frame_refs, "stats", &mut buf).unwrap();
    let ds = DataSource::new(buf);
    let mut dec: MveDecoder<Box<dyn infinitier_datasource::DataTrait>> =
        MveDecoder::new(ds.reader().unwrap(), "stats").unwrap();
    while dec.next_frame().unwrap().is_some() {}
    dec.block_mode_stats().video16
}

fn assert_uses_mode(width: u16, height: u16, frames: &[Vec<u16>], opcode: usize) {
    let stats = block_mode_stats_for(width, height, frames);
    assert!(
        stats[opcode] >= 1,
        "expected at least one 0x{opcode:x} block, got: {stats:?}"
    );
    assert_round_trip_bit_exact(width, height, frames);
}

// ─── 0x7 (2-colour) ─────────────────────────────────────────────────────────

#[test]
fn mode_0x7_per_2x2_two_colours_2x2_uniform() {
    // 2 colours, every 2×2 sub-block uniform → 0x7 per-2×2 (6 B).
    let a = pack_rgb555(0xff, 0, 0);
    let b = pack_rgb555(0, 0, 0xff);
    let mut block = vec![0u16; 64];
    let mut y = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = if ((x / 2) ^ (y / 2)) & 1 == 0 { a } else { b };
            for dy in 0..2 {
                for dx in 0..2 {
                    block[(y + dy) * 8 + x + dx] = v;
                }
            }
            x += 2;
        }
        y += 2;
    }
    assert_uses_mode(8, 8, &[block], 0x7);
}

#[test]
fn mode_0x7_per_row_two_colours_arbitrary() {
    // 2 colours but NOT 2×2 uniform → 0x7 per-row (12 B).
    let a = pack_rgb555(0xff, 0, 0);
    let b = pack_rgb555(0, 0xff, 0);
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        for x in 0..8 {
            // Diagonal stripes: pattern that breaks every 2×2.
            block[y * 8 + x] = if (x + y) & 1 == 0 { a } else { b };
        }
    }
    assert_uses_mode(8, 8, &[block], 0x7);
}

// ─── 0x9 (3-4 colour) ───────────────────────────────────────────────────────

#[test]
fn mode_0x9_per_2x2_four_colours_2x2_uniform() {
    // 4 colours, every 2×2 uniform → 0x9 per-2×2 (12 B).
    let cols = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
        pack_rgb555(0xff, 0xff, 0),
    ];
    let mut block = vec![0u16; 64];
    let mut y = 0;
    let mut idx = 0usize;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = cols[idx % 4];
            idx += 1;
            for dy in 0..2 {
                for dx in 0..2 {
                    block[(y + dy) * 8 + x + dx] = v;
                }
            }
            x += 2;
        }
        y += 2;
    }
    assert_uses_mode(8, 8, &[block], 0x9);
}

#[test]
fn mode_0x9_per_2x1_three_colours_wide_pairs() {
    // 3 distinct colours, every 2×1 (wide) pair uniform but
    // not every 2×2 uniform → 0x9 per-2×1 (16 B).
    let cols = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
    ];
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        let mut x = 0;
        while x < 8 {
            // Pick a colour for this 2-pixel wide segment, varying
            // by (x, y) so neighbouring rows differ → not 1×2 uniform,
            // and varying within 2×2 chunks vertically → not 2×2 uniform.
            let v = cols[((x / 2) + y) % 3];
            block[y * 8 + x] = v;
            block[y * 8 + x + 1] = v;
            x += 2;
        }
    }
    assert_uses_mode(8, 8, &[block], 0x9);
}

#[test]
fn mode_0x9_per_1x2_three_colours_tall_pairs() {
    let cols = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
    ];
    let mut block = vec![0u16; 64];
    let mut y = 0;
    while y < 8 {
        for x in 0..8 {
            let v = cols[(x + (y / 2)) % 3];
            block[y * 8 + x] = v;
            block[(y + 1) * 8 + x] = v;
        }
        y += 2;
    }
    assert_uses_mode(8, 8, &[block], 0x9);
}

#[test]
fn mode_0x9_per_pixel_four_colours_arbitrary() {
    // 4 distinct colours, no useful uniformity → 0x9 per-pixel (24 B).
    let cols = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
        pack_rgb555(0xff, 0xff, 0),
    ];
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        for x in 0..8 {
            // Pseudo-random of (x, y) that breaks 2×2, 2×1 and 1×2 uniformity.
            let i = (x * 7 + y * 11) & 3;
            block[y * 8 + x] = cols[i];
        }
    }
    assert_uses_mode(8, 8, &[block], 0x9);
}

// ─── 0x8 (2-colour per partition, half-split + per-quadrant) ────────────────

#[test]
fn mode_0x8_vertical_halves() {
    // Left half: 2 colours; right half: 2 different colours; 4 total
    // distinct → blocks the 0x7 paths and forces 0x8 vertical halves.
    let a = pack_rgb555(0xff, 0, 0);
    let b = pack_rgb555(0xff, 0xff, 0);
    let c = pack_rgb555(0, 0xff, 0);
    let d = pack_rgb555(0, 0, 0xff);
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        for x in 0..4 {
            block[y * 8 + x] = if (x + y) & 1 == 0 { a } else { b };
        }
        for x in 4..8 {
            block[y * 8 + x] = if (x + y) & 1 == 0 { c } else { d };
        }
    }
    assert_uses_mode(8, 8, &[block], 0x8);
}

#[test]
fn mode_0x8_horizontal_halves() {
    let a = pack_rgb555(0xff, 0, 0);
    let b = pack_rgb555(0xff, 0xff, 0);
    let c = pack_rgb555(0, 0xff, 0);
    let d = pack_rgb555(0, 0, 0xff);
    let mut block = vec![0u16; 64];
    for y in 0..4 {
        for x in 0..8 {
            block[y * 8 + x] = if (x + y) & 1 == 0 { a } else { b };
        }
    }
    for y in 4..8 {
        for x in 0..8 {
            block[y * 8 + x] = if (x + y) & 1 == 0 { c } else { d };
        }
    }
    assert_uses_mode(8, 8, &[block], 0x8);
}

#[test]
fn mode_0x8_per_quadrant() {
    // Each 4×4 quadrant: 2 distinct colours; total 8 distinct →
    // forces the 24-byte per-quadrant sub-mode (the half-split paths
    // each need ≤ 2 colours per half).
    let cols = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0xff, 0xff, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0xff, 0xff),
        pack_rgb555(0, 0, 0xff),
        pack_rgb555(0xff, 0, 0xff),
        pack_rgb555(0xff, 0x80, 0),
        pack_rgb555(0x80, 0x80, 0x80),
    ];
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        for x in 0..8 {
            let q = (if y < 4 { 0 } else { 2 }) + (if x < 4 { 0 } else { 1 });
            // Within each quadrant, pick between 2 colours from the
            // pair allocated to that quadrant.
            let pair_base = q * 2;
            let v = cols[pair_base + ((x + y) & 1)];
            block[y * 8 + x] = v;
        }
    }
    assert_uses_mode(8, 8, &[block], 0x8);
}

// ─── 0xa (4-colour per partition, half-split + per-quadrant) ────────────────

#[test]
fn mode_0xa_vertical_halves() {
    // Left half: 4 colours; right half: 4 different colours; 8 total →
    // 0xa vertical halves (32 B).
    let cols: [u16; 8] = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
        pack_rgb555(0xff, 0xff, 0),
        pack_rgb555(0xff, 0, 0xff),
        pack_rgb555(0, 0xff, 0xff),
        pack_rgb555(0xff, 0x80, 0),
        pack_rgb555(0x80, 0x80, 0x80),
    ];
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        for x in 0..4 {
            // Left half — pseudo-random of (x, y) over 4 colours;
            // breaks 2×1/1×2 and 0x8 (which would need ≤ 2 colours/half).
            let i = (x * 5 + y * 3) & 3;
            block[y * 8 + x] = cols[i];
        }
        for x in 4..8 {
            let i = ((x - 4) * 5 + y * 3) & 3;
            block[y * 8 + x] = cols[4 + i];
        }
    }
    assert_uses_mode(8, 8, &[block], 0xa);
}

#[test]
fn mode_0xa_horizontal_halves() {
    let cols: [u16; 8] = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
        pack_rgb555(0xff, 0xff, 0),
        pack_rgb555(0xff, 0, 0xff),
        pack_rgb555(0, 0xff, 0xff),
        pack_rgb555(0xff, 0x80, 0),
        pack_rgb555(0x80, 0x80, 0x80),
    ];
    let mut block = vec![0u16; 64];
    for y in 0..4 {
        for x in 0..8 {
            let i = (x * 5 + y * 3) & 3;
            block[y * 8 + x] = cols[i];
        }
    }
    for y in 4..8 {
        for x in 0..8 {
            let i = (x * 5 + (y - 4) * 3) & 3;
            block[y * 8 + x] = cols[4 + i];
        }
    }
    assert_uses_mode(8, 8, &[block], 0xa);
}

#[test]
fn mode_0xa_per_quadrant() {
    // Each 4×4 quadrant: up to 4 distinct colours, total 16 distinct.
    // Half-split paths (≤ 4/half) and per-quadrant 0x8 (≤ 2/quadrant)
    // both fail → 0xa per-quadrant (48 B) is the cheapest fit.
    let cols: [u16; 16] = [
        pack_rgb555(0x10, 0x00, 0x00),
        pack_rgb555(0x20, 0x00, 0x00),
        pack_rgb555(0x30, 0x00, 0x00),
        pack_rgb555(0x40, 0x00, 0x00),
        pack_rgb555(0x00, 0x10, 0x00),
        pack_rgb555(0x00, 0x20, 0x00),
        pack_rgb555(0x00, 0x30, 0x00),
        pack_rgb555(0x00, 0x40, 0x00),
        pack_rgb555(0x00, 0x00, 0x10),
        pack_rgb555(0x00, 0x00, 0x20),
        pack_rgb555(0x00, 0x00, 0x30),
        pack_rgb555(0x00, 0x00, 0x40),
        pack_rgb555(0x10, 0x10, 0x00),
        pack_rgb555(0x20, 0x20, 0x00),
        pack_rgb555(0x30, 0x30, 0x00),
        pack_rgb555(0x40, 0x40, 0x00),
    ];
    let mut block = vec![0u16; 64];
    for y in 0..8 {
        for x in 0..8 {
            let q = (if y < 4 { 0 } else { 2 }) + (if x < 4 { 0 } else { 1 });
            // Within each quadrant, pseudo-random pick from its 4 colours.
            let local_x = x & 3;
            let local_y = y & 3;
            let i = (local_x * 5 + local_y * 3) & 3;
            block[y * 8 + x] = cols[q * 4 + i];
        }
    }
    assert_uses_mode(8, 8, &[block], 0xa);
}

// ─── chooser cost ordering — cheaper modes preferred when applicable ────────

#[test]
fn solid_block_prefers_0xe_over_0x7() {
    // A genuinely uniform block must hit 0xe (2 B), not 0x7 (6 B).
    let red = pack_rgb555(0xff, 0, 0);
    let block = vec![red; 64];
    let stats = block_mode_stats_for(8, 8, &[block]);
    assert_eq!(stats[0xe], 1);
    assert_eq!(stats[0x7], 0);
}

#[test]
fn quad_pattern_prefers_0x9_over_0xc() {
    // 3 distinct colours, every 2×2 uniform: 0x9 per-2×2 (12 B) wins
    // over 0xc 4×4 fill (32 B).
    let a = pack_rgb555(0xff, 0, 0);
    let b = pack_rgb555(0, 0xff, 0);
    let c = pack_rgb555(0, 0, 0xff);
    let mut block = vec![0u16; 64];
    let mut y = 0;
    let mut i = 0;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = match i % 3 {
                0 => a,
                1 => b,
                _ => c,
            };
            i += 1;
            for dy in 0..2 {
                for dx in 0..2 {
                    block[(y + dy) * 8 + x + dx] = v;
                }
            }
            x += 2;
        }
        y += 2;
    }
    let stats = block_mode_stats_for(8, 8, &[block]);
    assert_eq!(stats[0x9], 1);
    assert_eq!(stats[0xc], 0);
}

// ─── bit-15 safety: sentinel pixels avoid the 0x7/0x8/0x9/0xa modes ─────────

#[test]
fn bit15_set_pixel_falls_back_to_lossless_modes() {
    // A block with bit 15 set in some pixels would lose data through
    // the per-row 0x7 path (decoder strips bit 15 in the marker
    // position); the chooser must avoid those modes and reach 0xb.
    let mut block = vec![0u16; 64];
    for (i, slot) in block.iter_mut().enumerate() {
        *slot = if i & 1 == 0 { 0x8123 } else { 0x4567 };
    }
    let stats = block_mode_stats_for(8, 8, &[block.clone()]);
    assert_eq!(
        stats[0x7], 0,
        "must not pick 0x7 when bit 15 is set in any pixel"
    );
    assert_eq!(stats[0x8], 0);
    assert_eq!(stats[0x9], 0);
    assert_eq!(stats[0xa], 0);
    // Should have fallen through to 0xb (raw, lossless on any input).
    assert!(stats[0xb] >= 1);
    assert_round_trip_bit_exact(8, 8, &[block]);
}

// ─── lossy_downsample fallback ──────────────────────────────────────────────

use infinitier_mve_encoder::encode_video_rgb555_lossy;

fn block_mode_stats_lossy(width: u16, height: u16, frames: &[Vec<u16>]) -> [u64; 16] {
    let frame_refs: Vec<&[u16]> = frames.iter().map(|f| f.as_slice()).collect();
    let mut buf = Vec::new();
    encode_video_rgb555_lossy(width, height, 66_667, &frame_refs, "lossy", &mut buf).unwrap();
    let ds = DataSource::new(buf);
    let mut dec: MveDecoder<Box<dyn infinitier_datasource::DataTrait>> =
        MveDecoder::new(ds.reader().unwrap(), "lossy").unwrap();
    while dec.next_frame().unwrap().is_some() {}
    dec.block_mode_stats().video16
}

#[test]
fn lossy_downsample_replaces_raw_with_4x4_fill() {
    // 64 distinct RGB555 values, no useful uniformity → strictly
    // lossless emits 0xb (128 B). With lossy_downsample = true the
    // chooser falls through to 0xc (32 B) instead.
    let mut block = vec![0u16; 64];
    for i in 0..64u16 {
        let r = (i & 0x1f) as u8;
        let b = ((i >> 5) & 0x07) as u8;
        block[i as usize] = (((r as u16) << 10) | (b as u16)) & 0x7fff;
    }
    // Sanity: lossless path picks 0xb.
    let lossless_stats = block_mode_stats_for(8, 8, &[block.clone()]);
    assert_eq!(lossless_stats[0xb], 1);
    assert_eq!(lossless_stats[0xc], 0);

    // Lossy path picks 0xc instead.
    let lossy_stats = block_mode_stats_lossy(8, 8, &[block]);
    assert_eq!(lossy_stats[0xb], 0);
    assert_eq!(lossy_stats[0xc], 1);
}

#[test]
fn lossy_downsample_does_not_steal_from_lossless_modes() {
    // Block that 0x9 per-2×2 (lossless 12 B) can encode — even with
    // lossy_downsample=true the chooser must prefer the lossless
    // mode over 0xc lossy (32 B).
    let cols = [
        pack_rgb555(0xff, 0, 0),
        pack_rgb555(0, 0xff, 0),
        pack_rgb555(0, 0, 0xff),
        pack_rgb555(0xff, 0xff, 0),
    ];
    let mut block = vec![0u16; 64];
    let mut y = 0;
    let mut idx = 0usize;
    while y < 8 {
        let mut x = 0;
        while x < 8 {
            let v = cols[idx % 4];
            idx += 1;
            for dy in 0..2 {
                for dx in 0..2 {
                    block[(y + dy) * 8 + x + dx] = v;
                }
            }
            x += 2;
        }
        y += 2;
    }
    let stats = block_mode_stats_lossy(8, 8, &[block]);
    assert_eq!(stats[0x9], 1, "lossless 0x9 must still win");
    assert_eq!(stats[0xc], 0);
}
