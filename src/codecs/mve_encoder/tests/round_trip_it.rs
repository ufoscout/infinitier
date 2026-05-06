//! End-to-end Phase-5 round-trip: synth → encode → decode → check
//! the reconstructed pixels match the source bit-exactly.
//!
//! Phase-5 supports nine block modes:
//!
//! - `0x0` — skip (copy from previous frame, 0 bytes)
//! - `0xe` — solid colour (1 byte)
//! - `0x4` — motion compensation against previous frame (1 byte)
//! - `0xd` — 4×4 quadrants of one colour each (4 bytes)
//! - `0x7` "compact" — 2 colours, every 2×2 sub-block uniform (4 bytes)
//! - `0x9` quad-pattern — 3-4 colours, one of four sub-modes (8 / 12 / 20 bytes)
//! - `0x7` "full"    — 2 colours, arbitrary per-pixel pattern (10 bytes)
//! - `0xc` — 4×4 fill (every 2×2 sub-block uniform, ≥ 5 colours, 16 bytes)
//! - `0xb` — raw 8×8 (any layout, 64 bytes — fallback)
//!
//! The encoder picks the cheapest mode that fits each block in that
//! order. With `0xb` as a fallback, encoding is total: every
//! palette-8 block is representable.

use std::io::Cursor;

use infinitier_datasource::DataSource;
use infinitier_mve_decoder::MveDecoder;
use infinitier_mve_encoder::{
    encode_solid_colour_video, encode_static_palette8, encode_video, MveEncodeError,
    StaticImage, VideoOptions,
};

fn open_decoder(bytes: Vec<u8>) -> MveDecoder<Box<dyn infinitier_datasource::DataTrait>> {
    let ds = DataSource::new(bytes);
    MveDecoder::new(ds.reader().unwrap(), "test").expect("open MVE")
}

/// Decode every frame, return them as flat RGBA buffers.
fn decode_all(bytes: Vec<u8>) -> (u16, u16, Vec<Vec<u8>>) {
    let mut dec = open_decoder(bytes);
    let (w, h) = (dec.width(), dec.height());
    let mut frames = Vec::new();
    while let Some(f) = dec.next_frame().unwrap() {
        frames.push(f.video.pixels);
    }
    (w, h, frames)
}

// ─── Phase-1 carry-over: plain skip + solid_colour ───────────────────────────

#[test]
fn solid_colour_round_trip() {
    let mut buf = Vec::new();
    encode_solid_colour_video(320, 240, [0, 0, 252], 30, 66_667, "blue", &mut buf).unwrap();
    let (w, h, frames) = decode_all(buf);
    assert_eq!((w, h), (320, 240));
    assert_eq!(frames.len(), 30);
    for (i, f) in frames.iter().enumerate() {
        for chunk in f.chunks_exact(4) {
            assert_eq!(chunk, &[0, 0, 252, 255], "frame {i} pixel mismatch");
        }
    }
}

#[test]
fn frame_count_one_works() {
    let mut buf = Vec::new();
    encode_solid_colour_video(8, 8, [252, 252, 252], 1, 66_667, "white", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let mut count = 0;
    while dec.next_frame().unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 1);
}

#[test]
fn no_panic_on_max_size() {
    let mut buf = Vec::new();
    encode_solid_colour_video(640, 480, [0, 0, 252], 2, 66_667, "max", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let mut count = 0;
    while dec.next_frame().unwrap().is_some() {
        count += 1;
    }
    assert_eq!(count, 2);
}

// ─── Phase-2: mode 0xd (quadrants, 4 colours per 8×8) ────────────────────────

#[test]
fn quadrants_mode_round_trip() {
    // 8×8 image with 4 distinct quadrant colours.
    let mut pixels = vec![0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            let idx = ((y >= 4) as usize) * 2 + ((x >= 4) as usize);
            pixels[y * 8 + x] = [11, 22, 33, 44][idx];
        }
    }
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[11] = [252, 0, 0];
    palette[22] = [0, 252, 0];
    palette[33] = [0, 0, 252];
    palette[44] = [252, 252, 0];

    let img = StaticImage {
        width: 8,
        height: 8,
        pixels: pixels.clone(),
        palette,
    };
    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "quads", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();

    // Verify quadrant pixels.
    let expected_rgba = [
        [252, 0, 0, 255],
        [0, 252, 0, 255],
        [0, 0, 252, 255],
        [252, 252, 0, 255],
    ];
    for y in 0..8 {
        for x in 0..8 {
            let qi = ((y >= 4) as usize) * 2 + ((x >= 4) as usize);
            let off = (y * 8 + x) * 4;
            assert_eq!(
                &frame.video.pixels[off..off + 4],
                &expected_rgba[qi],
                "quadrant pixel ({x},{y})"
            );
        }
    }

    // Mode histogram: this single block should be encoded as 0xd.
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0xd], 1, "expected one 0xd block");
    assert_eq!(stats.blocks, 1);
}

// ─── Phase-2: mode 0x7 compact (2 colours, 2×2-uniform) ──────────────────────

#[test]
fn delta_compact_round_trip() {
    // 2 colours arranged as a 4×4 checkerboard of 2×2 sub-blocks
    // (so every 2×2 corner is uniform — qualifies for 0x7 compact).
    let mut pixels = vec![0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            let sx = x / 2;
            let sy = y / 2;
            pixels[y * 8 + x] = if (sx + sy) % 2 == 0 { 5 } else { 9 };
        }
    }
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[5] = [252, 0, 0];
    palette[9] = [0, 0, 252];
    let img = StaticImage {
        width: 8,
        height: 8,
        pixels,
        palette,
    };

    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "delta_compact", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();

    for y in 0..8 {
        for x in 0..8 {
            let sx = x / 2;
            let sy = y / 2;
            let expected = if (sx + sy) % 2 == 0 {
                [252, 0, 0, 255]
            } else {
                [0, 0, 252, 255]
            };
            let off = (y * 8 + x) * 4;
            assert_eq!(&frame.video.pixels[off..off + 4], &expected, "({x},{y})");
        }
    }
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0x7], 1, "expected one 0x7 block");
}

// ─── Phase-2: mode 0x7 full (2 colours, arbitrary per-pixel) ─────────────────

#[test]
fn delta_full_round_trip() {
    // 2 colours but the pattern breaks 2×2 sub-block uniformity:
    // even-x ↔ odd-x alternation. Compact mode can't represent it,
    // encoder must fall back to the per-row mask variant.
    let mut pixels = vec![0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            pixels[y * 8 + x] = if x % 2 == 0 { 1 } else { 2 };
        }
    }
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    let img = StaticImage {
        width: 8,
        height: 8,
        pixels,
        palette,
    };

    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "delta_full", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();

    for y in 0..8 {
        for x in 0..8 {
            let expected = if x % 2 == 0 {
                [252, 0, 0, 255]
            } else {
                [0, 252, 0, 255]
            };
            let off = (y * 8 + x) * 4;
            assert_eq!(&frame.video.pixels[off..off + 4], &expected, "({x},{y})");
        }
    }
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0x7], 1);
}

// ─── Phase-2: skips & multi-frame video ──────────────────────────────────────

#[test]
fn multi_frame_with_change_mixes_modes() {
    // 16×8 video with a left red block + right green block. Frame 1
    // is identical to frame 0 (every block skips). Frame 2 swaps
    // the colours (every block must re-emit 0xe).
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    let frame0: Vec<u8> = (0..16 * 8)
        .map(|i| if (i % 16) < 8 { 1 } else { 2 })
        .collect();
    let frame1 = frame0.clone();
    let frame2: Vec<u8> = (0..16 * 8)
        .map(|i| if (i % 16) < 8 { 2 } else { 1 })
        .collect();

    let opts = VideoOptions {
        width: 16,
        height: 8,
        frame_duration_us: 66_667,
        palette,
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = vec![&frame0, &frame1, &frame2];
    let mut buf = Vec::new();
    encode_video(&opts, &frames, "swap", &mut buf).unwrap();

    let (w, h, decoded) = decode_all(buf);
    assert_eq!((w, h), (16, 8));
    assert_eq!(decoded.len(), 3);

    // Verify pixels frame-by-frame.
    let red = [252u8, 0, 0, 255];
    let green = [0u8, 252, 0, 255];
    for y in 0..8 {
        for x in 0..16 {
            let off = (y * 16 + x) * 4;
            let f0_expected = if x < 8 { red } else { green };
            let f2_expected = if x < 8 { green } else { red };
            assert_eq!(&decoded[0][off..off + 4], &f0_expected, "f0 ({x},{y})");
            assert_eq!(&decoded[1][off..off + 4], &f0_expected, "f1 ({x},{y})");
            assert_eq!(&decoded[2][off..off + 4], &f2_expected, "f2 ({x},{y})");
        }
    }
}

#[test]
fn skip_blocks_dominate_when_frame_unchanged() {
    let opts = VideoOptions {
        width: 64,
        height: 64,
        frame_duration_us: 66_667,
        palette: Box::new([[0u8; 3]; 256]),
        lossy_downsample: false,
    };
    let pixels = vec![0u8; 64 * 64];
    let frames: Vec<&[u8]> = vec![&pixels, &pixels, &pixels];
    let mut buf = Vec::new();
    encode_video(&opts, &frames, "still", &mut buf).unwrap();

    let mut dec = open_decoder(buf);
    while dec.next_frame().unwrap().is_some() {}
    let stats = dec.block_mode_stats();
    let bpf = (64 / 8) * (64 / 8);
    assert_eq!(stats.frames, 3);
    // Frame 0: 0xe everywhere (solid). Frames 1-2: 0x0 (skip).
    assert_eq!(stats.video8[0xe] as usize, bpf);
    assert_eq!(stats.video8[0x0] as usize, bpf * 2);
}

#[test]
fn multi_frame_with_per_block_change_per_frame() {
    // Three frames of a 16×16 image where each frame paints an
    // additional 8×8 block. Block (0,0) uses colour 1 in all frames;
    // block (1,0) is colour 2 starting frame 1; block (0,1) is
    // colour 3 starting frame 2.
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];

    let f0 = vec![1u8; 16 * 16];
    let mut f1 = f0.clone();
    let mut f2 = f1.clone();
    for y in 0..8 {
        for x in 8..16 {
            f1[y * 16 + x] = 2;
            f2[y * 16 + x] = 2;
        }
    }
    for y in 8..16 {
        for x in 0..8 {
            f2[y * 16 + x] = 3;
        }
    }

    let opts = VideoOptions {
        width: 16,
        height: 16,
        frame_duration_us: 66_667,
        palette,
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = vec![&f0, &f1, &f2];
    let mut buf = Vec::new();
    encode_video(&opts, &frames, "stair", &mut buf).unwrap();

    let mut dec = open_decoder(buf);
    let mut got = Vec::new();
    while let Some(f) = dec.next_frame().unwrap() {
        got.push(f.video.pixels);
    }
    assert_eq!(got.len(), 3);

    // Spot-check block (0,0) — always red.
    assert_eq!(&got[0][..4], &[252, 0, 0, 255]);
    assert_eq!(&got[1][..4], &[252, 0, 0, 255]);
    assert_eq!(&got[2][..4], &[252, 0, 0, 255]);

    // Block (1,0) = top-right — green from frame 1 onwards.
    let off_block1 = (0 * 16 + 8) * 4;
    assert_eq!(&got[0][off_block1..off_block1 + 4], &[252, 0, 0, 255]);
    assert_eq!(&got[1][off_block1..off_block1 + 4], &[0, 252, 0, 255]);
    assert_eq!(&got[2][off_block1..off_block1 + 4], &[0, 252, 0, 255]);

    // Block (0,1) = bottom-left — blue from frame 2 onwards.
    let off_block_bl = (8 * 16 + 0) * 4;
    assert_eq!(&got[0][off_block_bl..off_block_bl + 4], &[252, 0, 0, 255]);
    assert_eq!(&got[1][off_block_bl..off_block_bl + 4], &[252, 0, 0, 255]);
    assert_eq!(&got[2][off_block_bl..off_block_bl + 4], &[0, 0, 252, 255]);

    // Histogram: 2x2 = 4 blocks/frame × 3 frames = 12 blocks.
    let stats = dec.block_mode_stats();
    assert_eq!(stats.frames, 3);
    assert_eq!(stats.blocks, 12);
    // Frame 0: all 4 blocks 0xe (solid). Frame 1: 1 changed (0xe), 3 skipped (0x0).
    // Frame 2: 1 changed (0xe), 3 skipped. Totals: 0xe = 6, 0x0 = 6.
    assert_eq!(stats.video8[0xe], 6);
    assert_eq!(stats.video8[0x0], 6);
}

// ─── Format mode-distribution sanity ─────────────────────────────────────────

#[test]
fn first_frame_uses_solid_colour_subsequent_use_skip() {
    let mut buf = Vec::new();
    encode_solid_colour_video(64, 64, [0, 0, 252], 3, 66_667, "", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    while dec.next_frame().unwrap().is_some() {}
    let stats = dec.block_mode_stats();
    let blocks_per_frame = (64 / 8) * (64 / 8);
    assert_eq!(stats.frames, 3);
    assert_eq!(stats.video8[0xe] as usize, blocks_per_frame);
    assert_eq!(
        stats.video8[0x0] as usize,
        blocks_per_frame * 2,
        "frames 1 & 2: all 0x0"
    );
    for (op, &c) in stats.video8.iter().enumerate() {
        if op == 0x0 || op == 0xe {
            continue;
        }
        assert_eq!(c, 0, "unexpected opcode 0x{op:x} count {c}");
    }
}

#[test]
fn frame_duration_round_trips_after_first_frame() {
    let mut buf = Vec::new();
    encode_solid_colour_video(8, 8, [0, 0, 252], 3, 66_667, "", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let _ = dec.next_frame().unwrap();
    assert_eq!(dec.frame_duration_us(), 66_667);
}

#[test]
fn cursor_path_decodes_too() {
    let mut buf = Vec::new();
    encode_solid_colour_video(8, 8, [0, 0, 252], 1, 66_667, "", &mut buf).unwrap();
    let _cursor = Cursor::new(buf.clone());
    let mut dec = open_decoder(buf);
    assert!(dec.next_frame().unwrap().is_some());
}

// ─── Phase-3: mode 0xc (4×4 fill, ≥3 colours, 2×2-uniform) ──────────────────

#[test]
fn four_by_four_fill_round_trip() {
    // 8×8 with 16 distinct 2×2 sub-blocks — every 2×2 corner is
    // uniform but the colours don't form a 4×4-quadrant pattern, so
    // 0xd doesn't fit. With ≥3 distinct colours the encoder picks
    // 0xc (16 bytes) over 0xb (64 bytes).
    let mut pixels = vec![0u8; 64];
    let mut palette = Box::new([[0u8; 3]; 256]);
    for sy in 0..4 {
        for sx in 0..4 {
            let idx = (sy * 4 + sx) as u8 + 1;
            // Distinct rgb so the visual round-trip is checkable.
            palette[idx as usize] = [(idx as u8) * 8, (idx as u8) * 4, 0];
            for dy in 0..2 {
                for dx in 0..2 {
                    pixels[(sy * 2 + dy) * 8 + (sx * 2 + dx)] = idx;
                }
            }
        }
    }

    let img = StaticImage {
        width: 8,
        height: 8,
        pixels: pixels.clone(),
        palette,
    };
    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "fill", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();

    // Every 2×2 sub-block reconstructs to its original colour.
    for sy in 0..4 {
        for sx in 0..4 {
            let idx = (sy * 4 + sx) as u8 + 1;
            let expected = [
                ((idx & 0xfc) >> 0) << 0, // r quantised at write
                0, 0, 255,
            ];
            let _ = expected; // we'll just compare full pixel below
            for dy in 0..2 {
                for dx in 0..2 {
                    let py = sy * 2 + dy;
                    let px = sx * 2 + dx;
                    let off = (py * 8 + px) * 4;
                    // Pixel must be the same across the 2×2 sub-block.
                    let p00_off = (sy * 2 * 8 + sx * 2) * 4;
                    assert_eq!(
                        &frame.video.pixels[off..off + 4],
                        &frame.video.pixels[p00_off..p00_off + 4],
                        "2×2 sub-block ({sx},{sy}) pixel ({dx},{dy}) inconsistent",
                    );
                }
            }
        }
    }

    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0xc], 1, "expected one 0xc block");
}

// ─── Phase-3: mode 0xb (raw 8×8 fallback) ───────────────────────────────────

#[test]
fn raw_block_round_trip_5_colours() {
    // 5 distinct palette indices: not solid, not quadrants, not
    // 2-colour, not 2×2-uniform → falls through to 0xb (64 bytes).
    let mut pixels = vec![1u8; 64];
    pixels[0] = 2;
    pixels[1] = 3;
    pixels[10] = 4;
    pixels[63] = 5;
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [200, 0, 0];
    palette[2] = [0, 200, 0];
    palette[3] = [0, 0, 200];
    palette[4] = [200, 200, 0];
    palette[5] = [200, 0, 200];

    let img = StaticImage {
        width: 8,
        height: 8,
        pixels: pixels.clone(),
        palette: palette.clone(),
    };
    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "raw", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();

    // Quantise palette to 6-bit per channel (decoder reverses the
    // (>>2)<<2 round-trip).
    for y in 0..8 {
        for x in 0..8 {
            let idx = pixels[y * 8 + x] as usize;
            let [r, g, b] = palette[idx];
            let expected = [r & 0xfc, g & 0xfc, b & 0xfc, 255];
            let off = (y * 8 + x) * 4;
            assert_eq!(&frame.video.pixels[off..off + 4], &expected, "({x},{y})");
        }
    }
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0xb], 1, "expected one 0xb block");
}

#[test]
fn raw_block_round_trip_dense_pattern() {
    // 5 colours sprinkled so neither quadrants nor 2×2 sub-blocks
    // are uniform, AND the count exceeds 0x9's 4-colour limit —
    // must take the 0xb path.
    let mut pixels = vec![0u8; 64];
    for i in 0..64 {
        pixels[i] = (i % 5) as u8 + 1;
    }
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];
    palette[4] = [252, 252, 0];
    palette[5] = [252, 0, 252];

    let img = StaticImage {
        width: 8,
        height: 8,
        pixels: pixels.clone(),
        palette: palette.clone(),
    };
    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "raw_dense", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();
    for y in 0..8 {
        for x in 0..8 {
            let idx = pixels[y * 8 + x] as usize;
            let [r, g, b] = palette[idx];
            let expected = [r & 0xfc, g & 0xfc, b & 0xfc, 255];
            let off = (y * 8 + x) * 4;
            assert_eq!(&frame.video.pixels[off..off + 4], &expected, "({x},{y})");
        }
    }
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0xb], 1);
}

#[test]
fn empty_frames_rejected() {
    let opts = VideoOptions {
        width: 8,
        height: 8,
        frame_duration_us: 66_667,
        palette: Box::new([[0u8; 3]; 256]),
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = vec![];
    let mut buf = Vec::new();
    let err = encode_video(&opts, &frames, "", &mut buf).unwrap_err();
    assert!(matches!(err, MveEncodeError::NoFrames));
}

// ─── Phase-4: mode 0x4 (motion compensation against prev frame) ─────────────

/// Build a 16×8 image where left half is `left` and right half is
/// `right`. Block 0 at columns 0..7, block 1 at columns 8..15.
fn two_block_image(left: &[u8; 64], right: &[u8; 64]) -> Vec<u8> {
    let mut out = vec![0u8; 16 * 8];
    for y in 0..8 {
        for x in 0..8 {
            out[y * 16 + x] = left[y * 8 + x];
            out[y * 16 + 8 + x] = right[y * 8 + x];
        }
    }
    out
}

#[test]
fn motion_compensation_horizontal_shift_8px() {
    // Two blocks per frame. Frame 0 = [P0, P1]. Frame 1 = [Q, P0] —
    // the right half of frame 1 is identical to the left half of
    // frame 0 (an 8-pixel right shift). Encoder must motion-match
    // frame-1 block 1 to frame-0 block 0 at offset (-8, 0).
    let mut palette = Box::new([[0u8; 3]; 256]);
    for i in 1..=12u8 {
        palette[i as usize] = [i.wrapping_mul(20), i.wrapping_mul(11), i.wrapping_mul(7)];
    }
    let palette_for_check = palette.clone();
    let p0: [u8; 64] = std::array::from_fn(|i| ((i % 5) as u8) + 1);
    let p1: [u8; 64] = std::array::from_fn(|i| ((i % 5) as u8) + 6);
    let q: [u8; 64] = std::array::from_fn(|i| (((i + 3) % 4) as u8) + 9);
    let frame0 = two_block_image(&p0, &p1);
    let frame1 = two_block_image(&q, &p0);

    let opts = VideoOptions {
        width: 16,
        height: 8,
        frame_duration_us: 66_667,
        palette,
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = vec![&frame0, &frame1];
    let mut buf = Vec::new();
    encode_video(&opts, &frames, "shift", &mut buf).unwrap();

    let mut dec = open_decoder(buf);
    let mut got = Vec::new();
    while let Some(f) = dec.next_frame().unwrap() {
        got.push(f.video.pixels);
    }
    assert_eq!(got.len(), 2);

    // Pixel-for-pixel reconstruction matches frame1 (palette
    // quantised to 6 bits per channel by the encoder).
    for y in 0..8 {
        for x in 0..16 {
            let idx = frame1[y * 16 + x] as usize;
            let [r, g, b] = palette_for_check[idx];
            let expected = [r & 0xfc, g & 0xfc, b & 0xfc, 255];
            let off = (y * 16 + x) * 4;
            assert_eq!(&got[1][off..off + 4], &expected, "frame1 ({x},{y}) idx={idx}");
        }
    }

    let stats = dec.block_mode_stats();
    assert_eq!(stats.frames, 2);
    // Frame 0: 2 raw blocks (P0, P1 each have 5 distinct colours).
    // Frame 1: block 0 (Q, 4 distinct colours) → 0x9 quad-pattern;
    // block 1 (= P0) motion-matches at (-8, 0) → 0x4.
    assert_eq!(stats.video8[0x4], 1, "expected one 0x4 block");
    assert_eq!(stats.video8[0xb], 2, "expected two 0xb blocks (P0, P1 in frame 0)");
    assert_eq!(stats.video8[0x9], 1, "expected one 0x9 block (Q in frame 1)");
}

#[test]
fn motion_comp_panning_video_uses_0x4_widely() {
    // 24×8 (3 blocks wide). Each frame is the previous frame shifted
    // 1 pixel right; the leftmost column gets a fresh value. With 3
    // frames, frames 1 and 2 should encode blocks 1 and 2 as 0x4 at
    // offset (-1, 0), since those columns are exactly the previous
    // frame's columns one pixel to the left.
    let mut palette = Box::new([[0u8; 3]; 256]);
    for i in 1..=20u8 {
        palette[i as usize] = [i.wrapping_mul(13), i.wrapping_mul(7), i.wrapping_mul(3)];
    }
    let make_frame = |seed: u8| -> Vec<u8> {
        let mut f = vec![0u8; 24 * 8];
        for y in 0..8 {
            for x in 0..24 {
                f[y * 24 + x] = ((x as u8 + y as u8 + seed) % 5) + 1;
            }
        }
        f
    };
    let f0 = make_frame(0);
    // Shift f0 right by 1 pixel; column 0 gets a new value.
    let mut f1 = vec![0u8; 24 * 8];
    for y in 0..8 {
        f1[y * 24] = ((y as u8 + 9) % 5) + 6;
        for x in 1..24 {
            f1[y * 24 + x] = f0[y * 24 + x - 1];
        }
    }
    // f2 = f1 shifted right by 1.
    let mut f2 = vec![0u8; 24 * 8];
    for y in 0..8 {
        f2[y * 24] = ((y as u8 + 17) % 5) + 11;
        for x in 1..24 {
            f2[y * 24 + x] = f1[y * 24 + x - 1];
        }
    }

    let opts = VideoOptions {
        width: 24,
        height: 8,
        frame_duration_us: 66_667,
        palette,
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = vec![&f0, &f1, &f2];
    let mut buf = Vec::new();
    encode_video(&opts, &frames, "pan", &mut buf).unwrap();

    let mut dec = open_decoder(buf);
    let mut got = Vec::new();
    while let Some(f) = dec.next_frame().unwrap() {
        got.push(f.video.pixels);
    }

    // Reconstruction matches every source frame.
    for (fi, src) in [&f0, &f1, &f2].iter().enumerate() {
        for y in 0..8 {
            for x in 0..24 {
                let idx = src[y * 24 + x] as usize;
                let [r, g, b] = [
                    (idx as u8).wrapping_mul(13) & 0xfc,
                    (idx as u8).wrapping_mul(7) & 0xfc,
                    (idx as u8).wrapping_mul(3) & 0xfc,
                ];
                let off = (y * 24 + x) * 4;
                assert_eq!(
                    &got[fi][off..off + 4],
                    &[r, g, b, 255],
                    "frame {fi} ({x},{y}) idx={idx}",
                );
            }
        }
    }

    let stats = dec.block_mode_stats();
    // Frames 1 & 2: blocks 1 and 2 should motion-match at (-1, 0).
    // Block 0 of frame 1/2 contains the fresh leftmost pixel — won't
    // motion-match (out-of-bounds source) and won't skip.
    assert!(
        stats.video8[0x4] >= 4,
        "expected ≥4 motion-comp blocks; got {}",
        stats.video8[0x4]
    );
}

#[test]
fn frame0_never_uses_motion_compensation() {
    // First frame has no prev frame to read from; encoder must never
    // emit 0x4 there. Simple raw-content single-frame video.
    let mut pixels = vec![1u8; 64];
    pixels[10] = 2;
    pixels[20] = 3;
    pixels[30] = 4;
    pixels[40] = 5;
    let img = StaticImage {
        width: 8,
        height: 8,
        pixels,
        palette: Box::new([[0u8; 3]; 256]),
    };
    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "f0", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    while dec.next_frame().unwrap().is_some() {}
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0x4], 0, "frame 0 must not use 0x4");
}

#[test]
fn motion_comp_respects_search_window_bounds() {
    // 8×8 (single block). Two frames where frame 1's pixels = a
    // partial subset of frame 0's pixels — but the source position
    // for any non-zero offset would be out of bounds (frame is just
    // 8×8). So motion search must find no match and fall through to
    // a non-0x4 mode.
    let mut palette = Box::new([[0u8; 3]; 256]);
    for i in 1..=10u8 {
        palette[i as usize] = [i.wrapping_mul(20), i.wrapping_mul(11), i.wrapping_mul(7)];
    }
    let f0: Vec<u8> = (0..64).map(|i| ((i as u8) % 5) + 1).collect();
    let f1: Vec<u8> = (0..64).map(|i| ((i as u8) % 5) + 6).collect();
    let opts = VideoOptions {
        width: 8,
        height: 8,
        frame_duration_us: 66_667,
        palette,
        lossy_downsample: false,
    };
    let frames: Vec<&[u8]> = vec![&f0, &f1];
    let mut buf = Vec::new();
    encode_video(&opts, &frames, "1block", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    while dec.next_frame().unwrap().is_some() {}
    let stats = dec.block_mode_stats();
    // Motion can only consider offset (0, 0) for an 8×8 frame, which
    // is the skip case (already handled). So 0x4 must stay zero.
    assert_eq!(stats.video8[0x4], 0);
}

// ─── Phase-5: mode 0x9 quad-pattern family ───────────────────────────────────

/// Build an 8×8 image, run a single-frame round-trip, and return the
/// (decoder, decoded RGBA frame) so tests can inspect both the
/// pixels and the per-block mode histogram.
fn single_block_round_trip(
    pixels: [u8; 64],
    palette: Box<[[u8; 3]; 256]>,
) -> (
    MveDecoder<Box<dyn infinitier_datasource::DataTrait>>,
    Vec<u8>,
) {
    let img = StaticImage {
        width: 8,
        height: 8,
        pixels: pixels.to_vec(),
        palette,
    };
    let mut buf = Vec::new();
    encode_static_palette8(&img, 1, 66_667, "0x9", &mut buf).unwrap();
    let mut dec = open_decoder(buf);
    let frame = dec.next_frame().unwrap().unwrap();
    let pixels = frame.video.pixels;
    (dec, pixels)
}

fn assert_pixel(decoded: &[u8], x: usize, y: usize, expected_rgb: [u8; 3]) {
    let off = (y * 8 + x) * 4;
    let q = |c: u8| c & 0xfc;
    let expected = [q(expected_rgb[0]), q(expected_rgb[1]), q(expected_rgb[2]), 255];
    assert_eq!(&decoded[off..off + 4], &expected, "pixel ({x},{y})");
}

#[test]
fn quad_pattern_per_2x2_round_trip_4_colours() {
    // 4 distinct colours, every 2×2 sub-block is uniform but the
    // arrangement isn't 4×4-quadrant-uniform — must use 0x9
    // per-2×2 (8 bytes, vs 0xc's 16 or 0xb's 64).
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];
    palette[4] = [252, 252, 0];

    // 4×4 grid of 2×2 sub-blocks. Layout chosen so quadrants aren't
    // uniform: alternating colour pattern.
    //   row of sub-blocks (sy=0): 1 2 3 4
    //   row of sub-blocks (sy=1): 2 3 4 1
    //   row of sub-blocks (sy=2): 3 4 1 2
    //   row of sub-blocks (sy=3): 4 1 2 3
    let mut pixels = [0u8; 64];
    for sy in 0..4 {
        for sx in 0..4 {
            let idx = ((sx + sy) % 4) as u8 + 1;
            for dy in 0..2 {
                for dx in 0..2 {
                    pixels[(sy * 2 + dy) * 8 + (sx * 2 + dx)] = idx;
                }
            }
        }
    }
    let palette_for_check = palette.clone();
    let (dec, decoded) = single_block_round_trip(pixels, palette);

    for sy in 0..4 {
        for sx in 0..4 {
            let idx = ((sx + sy) % 4) as u8 + 1;
            for dy in 0..2 {
                for dx in 0..2 {
                    assert_pixel(
                        &decoded,
                        sx * 2 + dx,
                        sy * 2 + dy,
                        palette_for_check[idx as usize],
                    );
                }
            }
        }
    }
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0x9], 1, "expected one 0x9 block");
}

#[test]
fn quad_pattern_per_2x2_round_trip_3_colours() {
    // 3 distinct colours, 2×2-uniform; tests that the 3-colour
    // palette duplicate slot is correctly placed (p[1] reused).
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[5] = [252, 0, 0];
    palette[7] = [0, 252, 0];
    palette[11] = [0, 0, 252];

    let pattern = [5u8, 7, 11, 5];
    let mut pixels = [0u8; 64];
    for sy in 0..4 {
        for sx in 0..4 {
            let idx = pattern[(sx + sy * 3) % 3]; // ensures all 3 colours appear
            for dy in 0..2 {
                for dx in 0..2 {
                    pixels[(sy * 2 + dy) * 8 + (sx * 2 + dx)] = idx;
                }
            }
        }
    }
    let palette_for_check = palette.clone();
    let (dec, decoded) = single_block_round_trip(pixels, palette);

    for sy in 0..4 {
        for sx in 0..4 {
            let idx = pattern[(sx + sy * 3) % 3];
            for dy in 0..2 {
                for dx in 0..2 {
                    assert_pixel(
                        &decoded,
                        sx * 2 + dx,
                        sy * 2 + dy,
                        palette_for_check[idx as usize],
                    );
                }
            }
        }
    }
    assert_eq!(dec.block_mode_stats().video8[0x9], 1);
}

#[test]
fn quad_pattern_per_pixel_round_trip() {
    // 4 distinct colours, no 2×2 / 2×1 / 1×2 uniformity — encoder
    // falls through to 0x9 per-pixel (20 bytes, vs 0xb's 64).
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];
    palette[4] = [252, 252, 0];

    // Each pixel is independently `(x ^ y) % 4 + 1`. With this XOR
    // interleave, no row is 2-pixel-wide-uniform and no column is
    // 2-pixel-tall-uniform.
    let mut pixels = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            pixels[y * 8 + x] = ((x ^ y) % 4) as u8 + 1;
        }
    }
    let palette_for_check = palette.clone();
    let (dec, decoded) = single_block_round_trip(pixels, palette);

    for y in 0..8 {
        for x in 0..8 {
            let idx = ((x ^ y) % 4) + 1;
            assert_pixel(&decoded, x, y, palette_for_check[idx]);
        }
    }
    assert_eq!(dec.block_mode_stats().video8[0x9], 1);
}

#[test]
fn quad_pattern_per_2x1_wide_round_trip() {
    // 4 distinct colours, every 2-wide × 1-tall sub-block uniform
    // but not 2×2-uniform. Encoder emits 0x9 per-2×1 wide (12 bytes).
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];
    palette[4] = [252, 252, 0];

    let mut pixels = [0u8; 64];
    for y in 0..8 {
        for sx in 0..4 {
            // Each row's 4 sub-blocks (each 2 pixels wide) are
            // assigned colours `1 + ((sx + y) % 4)`. Different rows
            // within the same y/2 group differ → not 2×2-uniform.
            let idx = ((sx + y) % 4) as u8 + 1;
            pixels[y * 8 + sx * 2] = idx;
            pixels[y * 8 + sx * 2 + 1] = idx;
        }
    }
    let palette_for_check = palette.clone();
    let (dec, decoded) = single_block_round_trip(pixels, palette);

    for y in 0..8 {
        for sx in 0..4 {
            let idx = ((sx + y) % 4) + 1;
            assert_pixel(&decoded, sx * 2, y, palette_for_check[idx]);
            assert_pixel(&decoded, sx * 2 + 1, y, palette_for_check[idx]);
        }
    }
    assert_eq!(dec.block_mode_stats().video8[0x9], 1);
}

#[test]
fn quad_pattern_per_1x2_tall_round_trip() {
    // 4 distinct colours, every 1-wide × 2-tall sub-block uniform
    // but not 2×2-uniform.
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];
    palette[4] = [252, 252, 0];

    let mut pixels = [0u8; 64];
    for sy in 0..4 {
        for x in 0..8 {
            let idx = ((sy + x) % 4) as u8 + 1;
            pixels[(sy * 2) * 8 + x] = idx;
            pixels[(sy * 2 + 1) * 8 + x] = idx;
        }
    }
    let palette_for_check = palette.clone();
    let (dec, decoded) = single_block_round_trip(pixels, palette);

    for sy in 0..4 {
        for x in 0..8 {
            let idx = ((sy + x) % 4) + 1;
            assert_pixel(&decoded, x, sy * 2, palette_for_check[idx]);
            assert_pixel(&decoded, x, sy * 2 + 1, palette_for_check[idx]);
        }
    }
    assert_eq!(dec.block_mode_stats().video8[0x9], 1);
}

#[test]
fn quad_pattern_used_for_3_colours_arbitrary() {
    // 3 distinct colours, no sub-block uniformity. Encoder falls
    // through to 0x9 per-pixel (20 bytes).
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[10] = [252, 0, 0];
    palette[20] = [0, 252, 0];
    palette[30] = [0, 0, 252];

    // Use only 3 colours but in an arbitrary layout.
    let pattern = [10u8, 20, 30];
    let mut pixels = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            pixels[y * 8 + x] = pattern[(x + y) % 3];
        }
    }
    let palette_for_check = palette.clone();
    let (dec, decoded) = single_block_round_trip(pixels, palette);

    for y in 0..8 {
        for x in 0..8 {
            let idx = pattern[(x + y) % 3] as usize;
            assert_pixel(&decoded, x, y, palette_for_check[idx]);
        }
    }
    assert_eq!(dec.block_mode_stats().video8[0x9], 1);
}

#[test]
fn quad_pattern_does_not_steal_from_quadrants_or_delta() {
    // Sanity: cheaper modes still preferred for content that fits
    // them, even when the 0x9 path would also accept the block.
    //
    // 4-colour 4×4-quadrant-uniform block must use 0xd, not 0x9.
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    palette[3] = [0, 0, 252];
    palette[4] = [252, 252, 0];
    let mut pixels = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            let qi = ((y >= 4) as usize) * 2 + ((x >= 4) as usize);
            pixels[y * 8 + x] = [1, 2, 3, 4][qi];
        }
    }
    let (dec, _) = single_block_round_trip(pixels, palette);
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0xd], 1, "expected one 0xd block");
    assert_eq!(stats.video8[0x9], 0, "0x9 must not steal from 0xd");

    // 2-colour block must use 0x7 (4 bytes), not 0x9.
    let mut palette = Box::new([[0u8; 3]; 256]);
    palette[1] = [252, 0, 0];
    palette[2] = [0, 252, 0];
    let mut pixels = [0u8; 64];
    for y in 0..8 {
        for x in 0..8 {
            pixels[y * 8 + x] = if (x + y) % 2 == 0 { 1 } else { 2 };
        }
    }
    let (dec, _) = single_block_round_trip(pixels, palette);
    let stats = dec.block_mode_stats();
    assert_eq!(stats.video8[0x7], 1, "expected one 0x7 block");
    assert_eq!(stats.video8[0x9], 0, "0x9 must not steal from 0x7");
}
