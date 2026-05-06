# infinitier_mve_encoder

Pure-Rust encoder for the Interplay MVE video format (the cutscene
container used by BG1, BG2, IWD, and PST originals).

## Status: Phase 4 — total palette-8 encoder + motion compensation

Phase 4 adds mode `0x4` (1-byte motion compensation against the
previous frame). Every encoded frame after frame 0 is searched
against the previous frame for an exact 8×8 block match within a
16×16 offset window, before falling through to the spatial-only
modes. This shrinks panning/scrolling content significantly without
affecting the lossless guarantee — only exact matches are accepted.

| opcode | name              | meaning                                                                  | bits/block |
|--------|-------------------|--------------------------------------------------------------------------|------------|
| `0x0`  | `copy_prev_block` | with `VIDEO_FLAG_DELTA` set: "stay the same as last frame"               | 0          |
| `0xe`  | `solid_colour`    | fill the 8×8 block with one palette index                                 | 8          |
| `0x4`  | `motion_prev`     | copy 8×8 block from previous frame at offset `(dx, dy) ∈ [-8, 7]²`        | 8          |
| `0xd`  | `quadrants`       | 4 colours, one per 4×4 quadrant of the 8×8 block                          | 32         |
| `0x7c` | `delta_compact`   | 2 colours; 16-bit per-2×2 mask (when every 2×2 sub-block is uniform)      | 32         |
| `0x7f` | `delta_full`      | 2 colours; 8 per-row 8-bit masks                                          | 80         |
| `0xc`  | `4x4_fill`        | every 2×2 sub-block uniform with ≥3 colours; one byte per 2×2 (16 bytes) | 128        |
| `0xb`  | `raw`             | 64 raw palette indices, one per pixel — always works                      | 512        |

(`0x7c` / `0x7f` are both opcode `0x7`; the decoder branches on the
ordering of the two colour bytes.)

The chooser picks the cheapest mode that fits each block, in order:
**skip → solid → motion_prev → quadrants → delta_compact → delta_full
→ 4×4_fill → raw**.

Modes still missing for size-optimal real-world MVE coverage:

- `0x8`–`0xa` — quad-pattern family (4×4 sub-blocks with 2-, 3-,
  or 4-colour patterns) — significantly cheaper than `0xb` on
  natural-image content with multi-colour textures
- `0x1`, `0x2`, `0x3`, `0x5`, `0xf` — rare/edge cases avi2mve never
  emits in our test matrix

These would only reduce file size; output remains correct without them.

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
    };
    let frame_a = vec![0u8; 64 * 64];
    let frame_b = vec![1u8; 64 * 64];
    let frames: Vec<&[u8]> = vec![&frame_a, &frame_a, &frame_b];
    let mut out = std::fs::File::create("flicker.mve")?;
    encode_video(&opts, &frames, "flicker", &mut out)?;
    Ok(())
}
```
