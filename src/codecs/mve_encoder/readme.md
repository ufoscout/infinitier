# infinitier_mve_encoder

Pure-Rust encoder for the Interplay MVE video format (the cutscene
container used by BG1, BG2, IWD, and PST originals).

## Provenance

This encoder was developed by **observing the bitstream produced by
existing tools** rather than from any official format specification:

- **Video bitstream (per-block opcodes, chunk + segment framing, palette layout, frame timing):** 
  reverse-engineered by running `avi2mve.exe` (tool by Abel / TeamX, 2003-04) against
  a matrix of synthetic AVI inputs and analysing the per-block mode
  histograms of its output.
- **Audio path (Interplay DPCM):**
  cross-referenced against FFmpeg's
  [`libavcodec/interplay_dpcm.c`](https://github.com/FFmpeg/FFmpeg/blob/master/libavcodec/interplay_dpcm.c),
  which contains the canonical lookup table and saturation rules for
  the format. 

  
## Disclaimer — research / interoperability use only

The Interplay MVE container and its per-block coding modes are a
**proprietary format** owned by Interplay Entertainment. This crate
is published purely as a **research and interoperability project**:
it exists so that classic Infinity Engine cutscenes can be inspected,
re-encoded for analysis, and round-tripped through a documented
codepath. **It is not endorsed by, affiliated with, or licensed from
Interplay.** Use it to study the format, build modding tools, or
contribute to open-source preservation efforts; do not use it to
produce or distribute content that infringes Interplay's
intellectual-property rights. The crate's GPL-3.0-or-later licence governs the
Rust source code in this repository — it does not, and cannot,
grant any rights over the underlying file format.

## Status: Phase 5 — full quad-pattern coverage

Phase 5 adds mode `0x9` (quad-pattern), which carries 4 palette
indices and a per-sub-block bit-mask. Four sub-modes are picked by
the byte ordering of `p[0..4]`:

| sub-mode      | branch                          | mask bits/sub-block | cost     |
|---------------|---------------------------------|---------------------|----------|
| per-2×2       | `p0 ≤ p1 && p2 > p3`            | 16 sub-blocks of 2×2 | 8 bytes  |
| per-2×1 wide  | `p0 > p1 && p2 ≤ p3`            | 32 sub-blocks of 2×1 | 12 bytes |
| per-1×2 tall  | `p0 > p1 && p2 > p3`            | 32 sub-blocks of 1×2 | 12 bytes |
| per-pixel     | `p0 ≤ p1 && p2 ≤ p3`            | 64 sub-blocks of 1×1 | 20 bytes |

Mode `0x9` covers any 3- or 4-colour 8×8 block; the encoder picks the
cheapest sub-mode that fits the layout. This replaces `0xc` (16 bytes)
and `0xb` (64 bytes) for those colour counts.

Full opcode catalogue:

| opcode | name              | meaning                                                                   | bytes/block |
|--------|-------------------|---------------------------------------------------------------------------|-------------|
| `0x0`  | `copy_prev_block` | with `VIDEO_FLAG_DELTA` set: "stay the same as last frame"                | 0           |
| `0xe`  | `solid_colour`    | fill the 8×8 block with one palette index                                 | 1           |
| `0x4`  | `motion_prev`     | copy 8×8 block from previous frame at offset `(dx, dy) ∈ [-8, 7]²`        | 1           |
| `0xd`  | `quadrants`       | 4 colours, one per 4×4 quadrant of the 8×8 block                          | 4           |
| `0x7c` | `delta_compact`   | 2 colours; 16-bit per-2×2 mask (when every 2×2 sub-block is uniform)      | 4           |
| `0x9 ` | `quad_pattern`    | 3-4 colours, one of four sub-modes; see table above                       | 8 / 12 / 20 |
| `0x7f` | `delta_full`      | 2 colours; 8 per-row 8-bit masks                                          | 10          |
| `0xc`  | `4x4_fill`        | every 2×2 sub-block uniform with ≥ 5 colours; one byte per 2×2 (16 bytes) | 16          |
| `0xb`  | `raw`             | 64 raw palette indices, one per pixel — always works                      | 64          |

The chooser picks the cheapest mode that fits each block, in order:
**skip → solid → motion_prev → quadrants → delta_compact → delta_full
→ quad_pattern → 4×4_fill → raw**.

Modes still missing (avi2mve never emits them in our test matrix):
`0x1`, `0x2`, `0x3`, `0x5`, `0x8`, `0xa`, `0xf`. The first four are
edge-case temporal modes (`0x1` keep-from-2-frames-ago, `0x2`/`0x3`
self-referential motion within the current frame, `0x5` 16-bit motion
offset). `0x8` and `0xa` are alternative quad-pattern variants with
different palette layouts but no clear size advantage over `0x9` in
practice. None affect correctness — they would only marginally
reduce file size.

## Performance note

Motion search is brute-force: for every block in every non-first
frame, the encoder walks all 256 candidates in the 16×16 window and
compares 8 rows of 8 bytes for equality. That is 16k byte comparisons
per block (or ~250M per second of 640×480 footage at 15 fps). It is
plenty for offline encoding of static or short cutscene content; for
high-resolution real-time use, fingerprint-based search would be the
next optimisation.

## Usage

### Solid-colour helper

```rust,no_run
use infinitier_mve_encoder::encode_solid_colour_video;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = std::fs::File::create("blue.mve")?;
    encode_solid_colour_video(
        320, 240,        // resolution (must be multiples of 8)
        [0, 0, 252],     // RGB (8-bit; quantised to 6-bit for MVE)
        30,              // frame count
        66_667,          // frame duration in µs (≈ 15 fps)
        "blue.mve",      // log label
        &mut out,
    )?;
    Ok(())
}
```

### Static palette-8 still

```rust,no_run
use infinitier_mve_encoder::{encode_static_palette8, StaticImage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = StaticImage {
        width: 320,
        height: 240,
        pixels: vec![0; 320 * 240], // every pixel is palette index 0
        palette: Box::new([[0u8; 3]; 256]),
    };
    let mut out = std::fs::File::create("static.mve")?;
    encode_static_palette8(&img, 30, 66_667, "static", &mut out)?;
    Ok(())
}
```

### Multi-frame video

```rust,no_run
use infinitier_mve_encoder::{encode_video, VideoOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = VideoOptions {
        width: 64,
        height: 64,
        frame_duration_us: 66_667,
        palette: Box::new([[0u8; 3]; 256]),
        lossy_downsample: false,
    };
    let frame_a = vec![0u8; 64 * 64];
    let frame_b = vec![1u8; 64 * 64];
    let frames: Vec<&[u8]> = vec![&frame_a, &frame_a, &frame_b];
    let mut out = std::fs::File::create("flicker.mve")?;
    encode_video(&opts, &frames, "flicker", &mut out)?;
    Ok(())
}
```
