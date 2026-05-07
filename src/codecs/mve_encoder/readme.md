# infinitier_mve_encoder

Pure-Rust encoder for the **Interplay MVE** movies.
This includes both the `Interplay MVE` video format and the
`Interplay DPCM` audio format used by Infinity-Engine cutscenes
shipped with the original Baldur's Gate 1, BG2,
Icewind Dale, and Planescape: Torment.

## What is this for?

If you want to *play* Infinity-Engine cutscenes, you need a decoder
(see `infinitier_mve_decoder`). This crate solves the opposite
problem: **producing** valid MVE files from your own video frames
and audio. It is a research project useful for:

- **Modding** — replace a stock cutscene in BG2 / PST with your own.
- **Preservation work** — re-encode salvaged frames into a
  documented, round-trippable codepath, then verify against the
  ground-truth decoder.
- **Tooling** — power converters that accept any modern video format
  upstream (PNG sequence, RGB888 buffers, AVI via ffmpeg) and emit
  classic-engine-compatible cutscenes downstream.
- **Studying** — study the format, build modding tools, or
  experiment with new encodings.

The encoder produces output that is bit-stream-equivalent to what
`avi2mve.exe` and the official Interplay `mcomp.exe` write..

## Quick start

The simplest starting point is **a directory of PNG frames plus an
optional WAV audio file**. The encoder handles palette generation,
audio compression, and bitstream framing for you.

```rust,no_run
use infinitier_mve_encoder::{
    encode_from_assets, fps_to_frame_duration_us, FromAssetsOptions,
};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let png_paths: Vec<PathBuf> = vec![
        "frames/0001.png".into(),
        "frames/0002.png".into(),
        "frames/0003.png".into(),
        // … one PNG per video frame, all the same size
    ];

    let opts = FromAssetsOptions {
        frame_duration_us: fps_to_frame_duration_us(15.0), // 15 fps
        lossy_downsample: false,   // strict-lossless; flip on for very dense content
        strict_palette: false,     // auto-quantise > 256 colours via median-cut
        output_name: "intro".into(),
    };

    let out = encode_from_assets(
        &png_paths,
        std::path::Path::new("audio.wav"),
        &opts,
        std::path::Path::new("./out"),
    )?;
    println!("wrote {}", out.display());
    Ok(())
}
```

If you have your frames in memory as RGB888 you can skip the PNG
step and call the in-memory truecolour API:

```rust,no_run
use infinitier_mve_encoder::{encode_video_truecolour, fps_to_frame_duration_us};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (w, h) = (320u16, 240u16);
    let frame_a: Vec<[u8; 3]> = vec![[255, 0, 0]; (w as usize) * (h as usize)];
    let frame_b: Vec<[u8; 3]> = vec![[0, 0, 255]; (w as usize) * (h as usize)];
    let frames: Vec<&[[u8; 3]]> = vec![&frame_a, &frame_b];

    let mut out = File::create("flash.mve")?;
    encode_video_truecolour(
        w,
        h,
        fps_to_frame_duration_us(15.0),
        &frames,
        "flash",
        &mut out,
    )?;
    Ok(())
}
```

## Picking a video format

The MVE container supports two pixel formats. The encoder exposes
both; pick whichever matches your source data.

| Format | Best for | Entry points |
|---|---|---|
| **Palette-8** (8 bpp, 256-colour palette) | Source has ≤ 256 colours, OR you accept median-cut quantisation | `encode_from_assets`, `encode_video`, `encode_av`, `encode_video_truecolour`, `encode_av_truecolour`, `encode_solid_colour_video`, `encode_static_palette8` |
| **HiColor / RGB555** (16 bpp) | True-colour source, no palette quantisation, larger files | `encode_from_assets_rgb555`, `encode_video_rgb555`, `encode_av_rgb555`, `encode_video_rgb555_lossy` |

Real shipped game cutscenes use Palette-8 (BG, BG2, IWD, original
PST in places) and HiColor (PST `cannon.mve`, late PST cutscenes).
gemrb plays both.

### Variant cheat-sheet

- Want **the easiest** thing? `encode_from_assets` (Palette-8) or
  `encode_from_assets_rgb555` (HiColor). PNG dir + optional WAV in,
  `.mve` file out.
- Have **frames already in RAM** and don't want a palette ceremony?
  `encode_video_truecolour` (auto-quantises) or
  `encode_video_rgb555` (use [`pack_rgb555`] to build the u16
  buffers).
- Need **audio**? Use the `_av_` variants, which take
  `Some((&AudioOptions, &[Vec<i16>]))`. The
  `encode_from_assets[_rgb555]` helpers handle audio for you when
  you pass a WAV path.
- Producing **a static still or solid colour**?
  `encode_static_palette8` and `encode_solid_colour_video` are
  convenience entry-points that don't require you to assemble
  per-frame buffers.
- Hitting the **65 535-byte segment cap** on extreme-detail content
  (random noise at high resolution)?  Set `lossy_downsample = true`
  on `VideoOptions` / `FromAssetsOptions`, or call
  `encode_video_rgb555_lossy`. The encoder then falls through to a
  `0xc` 4×4-fill (top-left of each 2×2 wins) instead of `0xb` raw.

## Frame timing & sane defaults

`frame_duration_us` is microseconds between consecutive frames.
Common rates:

| Rate | `frame_duration_us` |
|---|---|
| 15 fps (most BG / IWD / PST cutscenes) | `66_667` |
| 30 fps (high-rate BG2 cutscenes) | `33_333` |
| 12 fps | `83_333` |

Use [`fps_to_frame_duration_us`] in code to avoid memorising those.

## Provenance

This encoder was developed by **observing the bitstream produced by
existing tools** rather than from any official format specification:

- **Video bitstream** (per-block opcodes, chunk + segment framing,
  palette layout, frame timing): reverse-engineered by running
  `avi2mve.exe` (community tool, 2003-04) against a matrix of
  synthetic AVI inputs and analysing the per-block mode histograms
  of its output. The two-stream layout for HiColor and the
  `OC_VIDEO_MODE` flag values were cross-checked against the
  official Interplay `mcomp.exe` (under DOSBox) and against shipped
  PS:T cutscenes.
- **Audio path** (Interplay DPCM): cross-referenced against
  FFmpeg's
  [`libavcodec/interplay_dpcm.c`](https://github.com/FFmpeg/FFmpeg/blob/master/libavcodec/interplay_dpcm.c),
  which contains the canonical lookup table and saturation rules
  for the format.


## Disclaimer — research / interoperability use only

The Interplay MVE container and its per-block coding modes are a
**proprietary format** owned by Interplay Entertainment. This crate
is published purely as a **research and interoperability project**:
it exists so that classic Infinity-Engine cutscenes can be inspected,
re-encoded for analysis, and round-tripped through a documented
codepath. **It is not endorsed by, affiliated with, or licensed from
Interplay.** Use it to study the format, build modding tools, or
contribute to open-source preservation efforts; do not use it to
produce or distribute content that infringes Interplay's
intellectual-property rights. The crate's GPL-3.0-or-later licence
governs the Rust source code in this repository — it does not, and
cannot, grant any rights over the underlying file format.


## API at a glance

| Function | Pixel format | Audio | Output | Notes |
|---|---|---|---|---|
| [`encode_solid_colour_video`] | Palette-8 | – | `&mut W` | Single RGB triple, repeats N frames |
| [`encode_static_palette8`] | Palette-8 | – | `&mut W` | One static image, N frames |
| [`encode_video`] | Palette-8 (indexed) | – | `&mut W` | General-purpose multi-frame |
| [`encode_av`] | Palette-8 (indexed) | yes | `&mut W` | General-purpose multi-frame + audio |
| [`encode_video_truecolour`] | RGB888 | – | `&mut W` | Auto-quantises via median-cut |
| [`encode_av_truecolour`] | RGB888 | yes | `&mut W` | Auto-quantises + audio |
| [`encode_video_rgb555`] | RGB555 (u16) | – | `&mut W` | Bit-15-clean input, lossless |
| [`encode_av_rgb555`] | RGB555 (u16) | yes | `&mut W` | + audio + lossy toggle |
| [`encode_video_rgb555_lossy`] | RGB555 (u16) | – | `&mut W` | Forces `lossy_downsample = true` |
| [`encode_from_assets`] | Palette-8 (auto-quantise) | yes (WAV) | `Path` | PNG dir + WAV → file path |
| [`encode_from_assets_rgb555`] | RGB555 | optional (WAV) | `Path` | PNG dir + optional WAV → file path |

Helpers: [`fps_to_frame_duration_us`], [`pack_rgb555`],
[`quantise_to_palette8`].
