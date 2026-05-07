# Cutscene video formats per game

Survey of the cutscene-video file formats used by each Infinity Engine
titles, derived from a content-level scan
(magic-byte sniff on loose files, `Interplay MVE File\x1A` / `BIKi` /
EBML-magic grep across BIF archives) rather than from extensions —
because the games **don't always agree** on which extension means what.

## Summary table

| Game | Where movies live | Format | File magic | Count |
|---|---|---|---|---|
| **Baldur's Gate** (original) | `movies/MOVIES.BIF`, `MOVIECD1.BIF`–`MOVIECD5.BIF`, `MovieCD6.bif` (BIF archives) | **Interplay MVE** | `Interplay MVE File\x1A...` | ≈ 31 cutscenes |
| **Baldur's Gate II** (original) | `data/Movies/Movies.bif`, `MovIntro.bif`, `MovEnd.bif`, `MovCD3.bif`, `25Movies.bif`; plus `data/MovHD0.bif` (HD CD install) | **Interplay MVE** | `Interplay MVE File\x1A...` | ≈ 22 cutscenes |
| **Icewind Dale** (original) | `CD2/Data/MVEfile1.bif`, `MVEfile2.bif`; `CD3/Data/eMOV1.bif`, `eMOV2.bif` | **Interplay MVE** | `Interplay MVE File\x1A...` | ≈ 8 cutscenes |
| **Icewind Dale II** | `Data/*.mve`, `CD2/Data/*.mve` (loose, **not** packed in BIFs) | **Bink Video v1** ⚠️ — *despite* the `.mve` extension! | `BIKi` (= `42 49 4B 69`) | 7 files: WOTC, Nvidia, Credits, BISlogo, Intro, END, Middle |
| **Planescape: Torment** (original) | `data/movies2.bif`, `movies4.bif` | **Interplay MVE** | `Interplay MVE File\x1A...` | ≈ 20 cutscenes |
| **Baldur's Gate EE** | `movies/*.wbm` + `lang/<locale>/movies/*.wbm` + `lo/` (low-res) + `480/` mirrors | **WebM** (VP8/VP9 video + Vorbis audio, Matroska container) | `\x1A 45 DF A3` (EBML) | 14 unique cutscenes — **49 total** with locale + `lo`/`480` re-encodes |
| **Baldur's Gate II EE** | same `movies/*.wbm` layout | **WebM** | `\x1A 45 DF A3` (EBML) | 27 unique — **37 total** |
| **Icewind Dale EE** | same `movies/*.wbm` layout | **WebM** | `\x1A 45 DF A3` (EBML) | 16 unique — **51 total** |
| **Planescape: Torment EE** | only `lang/<locale>/movies/*.wbm` (per-language only, no shared root set) | **WebM** | `\x1A 45 DF A3` (EBML) | 12 unique × multiple languages = **24 total** |

## Key takeaways

### 1. The `.mve` extension is *not* a reliable format indicator.

IWD2 ships **Bink Video** files under the `.mve` extension. The Infinity
Engine's loader matches resource types by **extension**, not by content,
so Black Isle could swap the underlying codec without touching the
engine. Every IWD2 cutscene sampled (`Nvidia.mve`, `Credits.mve`,
`BISlogo.mve`, `Intro.mve`, `END.mve`, `Middle.mve`, `WOTC.mve`)
starts with `42 49 4B 69` = `"BIKi"` = Bink Video v1, not the
expected `Interplay MVE File\x1A...`.

This means our `infinitier_mve_decoder` (Interplay MVE only) **cannot
play IWD2 cutscenes** — that would need a Bink decoder. FFmpeg's
`libavcodec/bink.c` is the only open-source Bink decoder; there is no
pure-Rust port today.

### 2. The BG / IWD / PST originals are all real Interplay MVE.

Packed inside BIF archives. Counts in the table above come from
`grep -ac "Interplay MVE File"` against each BIF — a reliable proxy
because every MVE file starts with that 26-byte signature. The MVEs
are extractable through any BIF tool (NearInfinity, our
`infinitier_bif_importer`, etc.) and then playable by
`infinitier_mve_decoder`.

### 3. Every Enhanced Edition cutscene is WebM under a `.wbm` extension.

Beamdog standardized on **libvpx (VP8 / VP9) + libvorbis** in a
**Matroska container** (`.webm` per the WebM spec, renamed to `.wbm`
for the engine's resource loader). Sampled files in BGEE, BG2EE,
IWDEE and PSTEE all start with `1A 45 DF A3` — the EBML / Matroska
file magic.

The folder layout is consistent across the four EEs:

```
movies/
  ├── *.wbm              # primary, full-resolution
  ├── lo/*.wbm           # low-res variants for older hardware
  └── 480/*.wbm          # 480p variants
lang/
  └── <locale>/
      └── movies/
          ├── *.wbm
          ├── lo/*.wbm
          └── 480/*.wbm
```

PSTEE is the outlier: it has *no* shared root `movies/` folder for
non-language-specific cutscenes — every clip lives under
`lang/<locale>/movies/`. The other EEs have both shared and localized
clips.

## Implications for our codec stack

| Layer | Coverage today |
|---|---|
| `infinitier_mve_decoder` | Plays Interplay MVE — covers **BG1, BG2, IWD, PST originals**. **Cannot** play IWD2 (`.mve` files there are Bink). |
| `infinitier_acm_decoder` | Plays ACM bitstreams (in-game audio for all classic titles). |
| `infinitier_wav_decoder` | Plays RIFF WAV, WAVC, and Ogg/Vorbis (the last via `symphonia` — already covers EE in-game audio under `.wav` extension that's actually OGG). |
| **Bink Video** | **Not implemented.** Needed to play IWD2 cutscenes. Closed format; only FFmpeg has open-source decoders. |
| **WebM (VP8/VP9 video)** | **Not implemented.** Needed to play *any* EE cutscene. Open format; pure-Rust decoders exist (e.g. `vpx-rs`, `dav1d` for AV1, FFmpeg via wrappers), but none are tiny / dependency-free. |

So:

- The classic-engine cutscenes (BG, BG2, IWD, PST) are fully covered
  end-to-end by code in this repo: BIF extract → `MveDecoder` → play.
- IWD2 needs a Bink decoder — historically nobody has open-source
  ported one to Rust.
- The Enhanced Editions need a WebM (VP8 / VP9 / Vorbis) demuxer +
  decoder. Vorbis we have; VP8/VP9 we don't.

## Reproducing the scan

Magic-byte sniffing on loose files:

```sh
head -c 16 /path/to/file.mve | xxd | head -1
```

- `49 6E 74 65 72 70 6C 61 79 20 4D 56 45 20 46 69` → "Interplay MVE Fi…" → real MVE
- `42 49 4B 69` → `BIKi` → Bink Video v1
- `42 49 4B 62` / `42 49 4B 66` → other Bink Video v1 sub-revisions
- `4B 42 32 ` → `KB2 ` → Bink Video v2
- `1A 45 DF A3` → EBML / Matroska / WebM
- `52 49 46 46` → `RIFF` → AVI or WAV (check the next 4 bytes)
- `53 4D 4B 32` / `53 4D 4B 34` → SMK2 / SMK4 → Smacker (older RAD format)

Counting MVE entries inside BIF archives (without parsing them):

```sh
grep -ac "Interplay MVE File" path/to/MOVIES.BIF
```

The 19-byte ASCII signature is unique enough that grep-counting it is
a reliable proxy for "this BIF holds N MVE resources."

---

# MVE encoder — current status and remaining phases

Notes for future-me (or a subsequent Claude session) picking up the
encoder work. Written after Phase 5 + audio support landed, so there
is real ground truth to validate against.

## Repo layout cheatsheet

```
src/codecs/mve_encoder/
  src/lib.rs              — Palette8 bitstream emitter + per-block chooser
  src/rgb555.rs           — HiColor (RGB555) bitstream emitter + chooser
  src/from_assets.rs      — high-level encode_from_assets API (Palette8)
  src/dpcm.rs             — Interplay DPCM audio compressor
  examples/encode_avi.rs  — CLI: ffmpeg → encode_video
  examples/build_test_assets.sh  → tools/build_mve_encoder_assets.sh
  tests/round_trip_it.rs                    — synthetic per-mode tests (Phase 1-6)
  tests/from_assets_round_trip.rs           — encode/decode every Palette8 asset folder
  tests/rgb555_round_trip.rs                — HiColor per-sub-mode round-trips + lossy fallback
  tests/from_assets_rgb555_round_trip.rs    — synth PNG → HiColor .mve → bit-exact decode
src/codecs/mve_decoder/
  src/decoder.rs          — chunk + segment loop, audio buffers
  src/video.rs            — every decode8_0xN / decode16_0xN function
  examples/block_mode_histogram.rs  — `cargo run --example block_mode_histogram <file.mve>`
tools/
  build_mve_encoder_assets.sh   — populates assets/mve_encoder/<name>/
  gemrb_mve_validator/          — cross-validator: feeds our output through
                                   gemrb's actual decoder primitives
                                   (`build_and_run.sh` rebuilds + runs the suite)
assets/mve_encoder/<name>/
  frame_NNNN.png      — paletted PNG
  audio.wav           — 22050 Hz mono 16-bit PCM
target/mve_encoder/<name>.mve   — produced by from_assets test
```

Reference fixtures avi2mve produced (the source of ground truth for
mode-distribution comparisons):
`tools/PS gui v3.04/PS gui (files)/mve_test/*.mve`

Quick commands:

```sh
# Run encoder unit + integration tests
cargo test --release -p infinitier_mve_encoder

# Compare mode distribution of our output vs avi2mve's
cargo run --release --example block_mode_histogram -p infinitier_mve_decoder -- \
  target/mve_encoder/320x240_15fps_3s_smptebars.mve
cargo run --release --example block_mode_histogram -p infinitier_mve_decoder -- \
  "tools/PS gui v3.04/PS gui (files)/mve_test/320x240_15fps_3s_smptebars.mve"
```

## What works (don't redo)

| Feature | Where |
|---|---|
| Container framing (signature, init/frame/end chunks) | `lib.rs::encode_av` |
| Palette-8 video, all chooser modes 0x0/0x4/0x7/0x8/0x9/0xa/0xb/0xc/0xd/0xe | `lib.rs::encode_block` |
| Lossy `0xc` fallback when raw would overflow segment cap | `lib.rs::build_4x4_fill_downsampled` |
| 16×16 brute-force motion search | `lib.rs::find_motion_match` |
| Multi-frame skip detection via `VIDEO_FLAG_DELTA` swap | `lib.rs::encode_av` |
| Interplay DPCM compressed audio (mono + stereo, ~halves audio bytes) | `dpcm.rs::compress`, `lib.rs::build_init_audio_chunk`, `build_frame_chunk` |
| `encode_from_assets` (PNG dir + WAV → .mve) | `from_assets.rs` |
| Round-trip integration tests across 10 fixtures | `tests/from_assets_round_trip.rs` |
| **OC_VIDEO_MODE flags fixed** — 0x0101 for PalColor, 0x0110 for HiColor (matches `mcomp.exe` / `avi2mve` / shipped game cutscenes) | `lib.rs::build_init_video_chunk`, `rgb555.rs::build_init_video_chunk_rgb555` |
| **HiColor (RGB555) encoder** — full mode parity with PalColor (0x0/0x4/0x7/0x8/0x9/0xa/0xb/0xc/0xd/0xe), bit-15 sub-mode selectors handled | `rgb555.rs` |
| **HiColor lossy fallback** — `lossy_downsample` toggle emits `0xc` (32 B) instead of `0xb` raw (128 B) for high-detail blocks, parity with the 8-bit path | `rgb555.rs::build_4x4_fill_downsampled_rgb555`, exposed via `encode_video_rgb555_lossy` and `encode_av_rgb555(..., lossy_downsample, ...)` |
| **HiColor `encode_from_assets_rgb555`** — PNG dir + optional WAV → 16-bit `.mve`. Reads RGB888 frames, packs each pixel via `pack_rgb555`. No `TooManyColours` failure mode (HiColor has no palette). | `from_assets.rs::encode_from_assets_rgb555` |
| **gemrb cross-validator** — compiles gemrb's actual `ipvideo_decode_frame{8,16}` + `ipaudio_uncompress` against our outputs | `tools/gemrb_mve_validator/` |

The encoder is **lossless on any palette-8 input** and **on any RGB555
input with bit 15 = 0** (the high bit is reserved by the format as a
sub-mode selector; `pack_rgb555` always produces values in [0, 0x7fff]
so this is automatic). High-detail content still benefits from
`lossy_downsample` (Palette8 only) to keep the file size reasonable.

## Phase 6 — alternative quad-pattern modes 0x8 + 0xa — **DONE**

**Goal**: emit modes `0x8` and `0xa` for natural-image content where
they beat the existing `0x9` per-pixel (20 bytes) + `0xb` raw (64
bytes) fallbacks.

**Status**: implemented in `lib.rs` as `build_0x8_per_quadrant`
(16 B), `build_0x8_vertical_halves` / `build_0x8_horizontal_halves`
(12 B each), `build_0xa_per_quadrant` (32 B),
`build_0xa_vertical_halves` / `build_0xa_horizontal_halves` (24 B
each). Per-mode round-trip tests live alongside the Phase-5 ones in
`tests/round_trip_it.rs::quadrant_pairs_*` /
`four_colour_*`.

### Final chooser order

Cost-sorted, with each mode's applicability constraint in parens.
Modes are tried top-down; the first one that fits wins:

| Step | Mode | Bytes | Constraint |
|---|---|---|---|
| 1 | `0x0` skip | 0 | block matches prev frame at same offset |
| 2 | `0xe` solid | 1 | every pixel identical |
| 3 | `0x4` motion | 1 | exact match within ±8 px of prev frame |
| 4 | `0xd` quadrants | 4 | 4-quadrant uniform |
| 5 | `0x7` compact | 4 | 2 colours, every 2×2 sub-block uniform |
| 6 | `0x9` per-2×2 | 8 | 3-4 colours, every 2×2 uniform |
| 7 | `0x7` full | 10 | 2 colours, arbitrary |
| 8 | `0x9` per-2×1 / per-1×2 | 12 | 3-4 colours + the appropriate 2-pixel uniformity |
| 9 | **`0x8` half-split** | 12 | each half (left/right or top/bottom) ≤ 2 colours |
| 10 | **`0x8` per-quadrant** | 16 | each 4×4 quadrant ≤ 2 colours |
| 11 | `0xc` 4×4-fill | 16 | every 2×2 sub-block uniform |
| 12 | `0x9` per-pixel | 20 | 3-4 colours total |
| 13 | **`0xa` half-split** | 24 | each half ≤ 4 colours |
| 14 | **`0xa` per-quadrant** | 32 | each 4×4 quadrant ≤ 4 colours |
| 15 | `0xc` lossy (opt-in) | 16 | always (top-left of each 2×2 wins) |
| 16 | `0xb` raw | 64 | always |

`0x8` half-split caps at 4 distinct colours total (≤ 2 × 2 halves), so
in the ≥ 5-colour branch only the per-quadrant variant can fire — the
implementation reflects that.

### Bit-layout traps that bit during implementation

- `pack_flags_8` lays the 32 mask bits as 4 rows × 8 cols. For the 0x8
  per-quadrant sub-mode, each quadrant's 16 bits live in two bytes
  (`b[lo]`/`b[hi]`); inside each byte the *low* nibble holds row 0 of
  the pair, the *high* nibble row 1. Easy to swap the nibble order
  by mistake.
- 0x8 sub-mode B/C selection is by **palette ordering**: `p[0] > p[1]`
  forces the half-split branch and `p[2] <= p[3]` (vs `p[2] > p[3]`)
  picks vertical vs horizontal. When a half is monochrome, the
  encoder must fabricate a phantom second palette entry (e.g. v=0
  → `pp0 = 1, pp1 = 0` and flip every bit) to keep the strict
  inequality. Covered by `quadrant_pairs_handles_palette_index_zero`.
- 0xa half-split mask layout differs from per-quadrant: vertical uses
  `b[y]` for x<4 / `b[y+8]` for x≥4, while horizontal uses `b[2y]`
  for x<4 / `b[2y+1]` for x≥4. Encoder has separate inner loops.

### Histogram comparison vs avi2mve (45 frames at 320×240)

| Asset | Ours size | avi2mve | Ours `0x8`% | avi2mve `0x8`% |
|---|---|---|---|---|
| smptebars | 165 KB | 166 KB | 0.0 % | 0.0 % |
| testsrc 320×240 | 128 KB | 140 KB | 0.16 % | 1.75 % |
| testsrc 160×120 | 132 KB | 134 KB | 0.08 % | 0.20 % |
| mandelbrot | 737 KB | 709 KB | 3.07 % | 3.20 % |
| noise (lossy) | 1008 KB | 968 KB | 37.58 % | 47.14 % |

We're competitive on natural-image content and *smaller* than avi2mve
on testsrc (`0x4` motion compensation does heavy lifting there).
The remaining size gap on mandelbrot / noise comes from avi2mve's
**lossy 0x7 / 0x9** paths — they pre-quantise each block to ≤ 2 / ≤ 4
"dominant" colours and accept the error, while our 0x8/0xa builders
are strictly lossless. That's a Phase-13 / future-work concern, not a
correctness issue.

### Bonus: lossless noise now fits the segment cap

Pre-Phase-6, encoding noise without `lossy_downsample` would emit
~32 K blocks of mode `0xb` (64 B each) per frame, busting the
65 535-byte segment limit. With 0x8 covering ~38 % of noise blocks
losslessly at 16 B and the remainder at 64 B, the per-frame video
segment now fits. Captured by
`tests/lossless_noise_check.rs::noise_encodes_losslessly_with_phase_6`
(produces a 2.5 MB lossless `noise_lossless.mve` — useful as a
regression sentinel even though `lossy_downsample = true` is still
the default for that fixture).

## Phase 7 — `0x5` 16-bit motion offset

**Goal**: extend motion compensation beyond `0x4`'s 16×16 window.

**Decoder reference**: `mve_decoder/src/video.rs:decode8_0x5` (lines
~126–138). Reads two i8 bytes `(x, y)` for the source offset — full
−128…127 range per axis.

**When it wins**: large pans (camera motion across a frame) where the
matching block in the previous frame is more than 8 pixels away.
Cost: 2 bytes vs `0x4`'s 1 byte, but unlocks much wider matches.

**Where**: insert between `0x4` (already implemented) and `0x7`/`0x9`
in the chooser. Search the wider window only if `0x4`'s 16×16 sweep
fails to find an exact match.

**Algorithm**:
1. After `0x4` fails, search `(dx, dy) ∈ [-128, 127]²` minus the
   already-searched 16×16 region for an exact 8×8 match.
2. Skip out-of-bounds candidates.
3. Brute force is 65 280 candidates per block — slow. Either accept
   the cost for offline encoding or build a content-hash index of
   the previous frame's blocks. A 64-bit FxHash or similar of
   `prev[(y, x)..(y+8, x+8)]` rows lets you reject most candidates
   in O(1).

**Validation**: synthesise a video where every block in frame 1 is
frame 0 shifted by say (30, 0) — outside the `0x4` window. Verify
the histogram shows `0x5` blocks and reconstruction is bit-exact.

**Estimated cost**: 1 session if you accept the brute-force search.
2–3 sessions if you build the hash index for performance.

## Phase 8 — temporal modes 0x1, 0x2, 0x3

**Goal**: cover the temporal-reference tail.

- `0x1` (`keep_2_frames_back`): copy the same block from 2 frames
  ago (0 bytes). Only usable when the frame-N-2 buffer survives —
  the decoder's two-buffer ring naturally provides this. Useful for
  flicker/blink animations.
- `0x2` / `0x3` (self-referential motion within current frame):
  copy from a block earlier in the same frame using a 1-byte
  offset. Useful for repeating UI elements / text.

**Decoder reference**:
- `0x1`: handled inline in `decode_frame8` (line ~565: just a
  no-op letting the existing `buf1[dst..]` data persist — but with
  `VIDEO_FLAG_DELTA` swap semantics this means "use 2-back").
- `0x2`: `decode8_0x2` (lines ~87–97), 1-byte offset, complex
  encoding via two halves.
- `0x3`: `decode8_0x3` (lines ~99–109), 1-byte offset with a
  different encoding scheme.

**When they win**: rarely. avi2mve emits `0x1` at 0.01% on noise and
0% on most other content. Real game cutscenes also barely use these.
Probably only worth implementing if chasing byte-exact reproduction
of avi2mve output.

**Estimated cost**: 1 session; defer unless byte-exactness is needed.

## Phase 9 — Interplay DPCM compressed audio — **DONE**

**Goal**: emit `AUDIO_FLAG_COMPRESSED` audio in the **Interplay DPCM**
format that avi2mve.exe produces. ~50% smaller than raw PCM (1 byte
per sample after the seed instead of 2). This is the codec ffprobe
reports as `interplay_dpcm` on real game cutscenes and on avi2mve
output; pre-Phase-9 we emitted `pcm_s16le` — interoperable but
visibly different in `ffprobe -show_streams`.

**Status**: implemented in `src/codecs/mve_encoder/src/dpcm.rs`
(`compress(samples, channels) -> Vec<u8>`). DPCM is the **only**
audio path the encoder produces — there is no toggle. The raw-PCM
branch was kept briefly while validating the compressor and then
removed once the bounded-error round-trip held across every fixture.
ffprobe now reports `codec_name=interplay_dpcm` on every output.

### Implementation notes

- **`DELTA_TABLE` is duplicated** into the encoder rather than
  re-exported from the decoder. The encoder doesn't otherwise depend
  on the decoder, and dragging in the dependency just to share 512
  bytes of static data wasn't worth it. The duplicate is one chunk
  of code; if the table ever changes (it shouldn't — it's defined by
  the format) keep both copies in sync.
- **Segment version stays at 1**, not 2 as the original Phase-9 plan
  suggested. avi2mve writes v1 with `flags = 0x0006`
  (`AUDIO_FLAG_16BIT | AUDIO_FLAG_COMPRESSED`); the decoder honours
  the COMPRESSED flag whenever `version > 0`, so v1 is sufficient and
  matches what real game cutscenes use.
- **`audio_size` in the per-frame `OC_AUDIO_DATA` header stays as the
  uncompressed byte count** (`samples.len() * 2`). The decoder uses
  this to size its output buffer; the *segment size* (which scales
  the compressed data) lives in the segment header one level up.
- The compressor uses a **linear 256-entry scan** per sample. The LUT
  is non-monotonic at indices 124–128 (`-1, 1, 1, 5481, -32589`),
  which would break a naïve binary search; offline encoding speed is
  not a bottleneck, so we leave it as a brute-force scan with an
  early-exit when `dist == 0`.
- For mono with N samples we write `2 + (N - 1)` bytes; for stereo
  with N (even) samples, `4 + (N - 2)`. Asserted in
  `dpcm::tests::output_length_matches_spec`.

### Reconstruction quality (measured)

| Source content | Per-sample Δ profile | Mean abs err | Max abs err |
|---|---|---|---|
| Silence | Δ = 0 | **0** (bit-exact) | 0 |
| Slow ramp (Δ ≤ 1) | LUT covers 1-LSB exactly | ≤ 1 | ≤ 1 |
| 1 kHz sine, ±4096 amp | mean Δ ≈ 326 (smptebars-like) | ≤ 30 | ≤ 256 |
| 4 kHz sine, ±20000 amp | mean Δ ≈ 4500 | ~1500 | ~8000 |

Typical Infinity Engine cutscene audio (dialogue, light music) sits
around the smptebars profile or smoother — well inside the budget the
existing decoder side has accepted for years.

### Size win across the fixture set

Audio is now ~half the bytes it used to be on every output. Sample
totals (post-Phase-6 + Phase-9):

| Fixture | Ours | avi2mve |
|---|---|---|
| smptebars 3 s | **99 KB** | 166 KB |
| testsrc 320×240 2 s | **84 KB** | 140 KB |
| testsrc 160×120 2 s | **88 KB** | 134 KB |
| mandelbrot 3 s | **670 KB** | 709 KB |
| noise 3 s (lossy video) | **942 KB** | 968 KB |

Every output reports `codec_name=interplay_dpcm` in
`ffprobe -select_streams a`.

### Validation harness

- `dpcm::tests::*` — empty input, mono/stereo seed-only, silence,
  slow ramp (≤ 1 LSB), 1 kHz sine (≤ 30 LSB mean), 4 kHz sine
  (loose bound), stereo channel-independence, output-length spec.
- `tests/from_assets_round_trip.rs` — every asset folder is encoded
  + decoded, and audio is checked against DPCM error bounds (mean
  ≤ 200 LSB, max ≤ 8192 LSB, length exact). Bounds are sized for
  the worst-case fixture (the noise WAV, with ~2700-LSB per-sample
  deltas); silence and smooth-tone fixtures sit two orders of
  magnitude inside them.
- ffprobe sanity check (manual):
  `ffprobe -select_streams a target/mve_encoder/320x240_15fps_3s_smptebars.mve`
  reports `codec_name=interplay_dpcm`.

## Phase 10 — RGB555 (16-bit) video — **DONE**

**Goal**: encode 16-bit-per-pixel RGB555 cutscenes. Real PS:T
`cannon.mve` and any `mcomp.exe HICOLOR.CFG` output is in this
format (`OC_VIDEO_BUFFERS.format_flag = 1`).

**Status**: implemented in `src/codecs/mve_encoder/src/rgb555.rs`.
Public API: `encode_video_rgb555`, `encode_av_rgb555`,
`pack_rgb555(r, g, b) -> u16`. Init chunk emits
`OC_VIDEO_MODE` w=640, h=480, **flags=0x0110** and `OC_VIDEO_BUFFERS`
v2 with **format_flag=1**; no `OC_PALETTE` segment.

### Mode coverage

All modes ported from the 8-bit chooser, in cost-sorted order:

| Step | Mode | Bytes | Constraint |
|---|---|---|---|
|   1 | `0x0` skip       | 0   | matches prev frame at same offset |
|   2 | `0xe` solid      | 2   | every pixel identical |
|   3 | `0x4` motion     | 1*  | exact match within ±8 px (\* in motion stream) |
|   4 | `0x7` per-2×2    | 6   | 2 colours, every 2×2 uniform |
|   5 | `0xd` quadrants  | 8   | each 4×4 quadrant uniform |
|   6 | `0x7` per-row    | 12  | 2 colours arbitrary |
|   7 | `0x9` per-2×2    | 12  | 3-4 colours, every 2×2 uniform |
|   8 | `0x9` per-2×1 / per-1×2 | 16  | 3-4 colours + 2×1 / 1×2 uniformity |
|   9 | `0x8` half-split | 16  | each half ≤ 2 colours |
|  10 | `0x9` per-pixel  | 24  | 3-4 colours arbitrary |
|  11 | `0x8` per-quadrant | 24  | each 4×4 quadrant ≤ 2 colours |
|  12 | `0xa` half-split | 32  | each half ≤ 4 colours |
|  13 | `0xc` 4×4 fill   | 32  | every 2×2 uniform (any colour count) |
|  14 | `0xa` per-quadrant | 48 | each 4×4 quadrant ≤ 4 colours |
| 14a | `0xc` lossy (opt-in via `lossy_downsample`) | 32 | always (top-left of each 2×2 wins) |
|  15 | `0xb` raw        | 128 | always (lossless fallback) |

The `0xc` lossy path (step 14a) is gated by `lossy_downsample = true`
on the encoder API. With it disabled, dense blocks fall through to
`0xb` raw at 128 B; with it enabled the chooser emits `0xc` at 32 B
(taking the top-left pixel of each 2×2 sub-block) and accepts the
loss. Required for high-detail content (e.g. random noise at
640×480) whose strictly-lossless raw form would exceed MVE's
65 535-byte segment cap.

### How to encode

```rust
// Strictly-lossless on bit-15-clean input:
encode_video_rgb555(width, height, frame_dur_us, &frames, "name", &mut out)?;

// With the lossy 0xc fallback (use only when raw would overflow):
encode_video_rgb555_lossy(width, height, frame_dur_us, &frames, "name", &mut out)?;

// Full-control variant (audio + lossy toggle):
encode_av_rgb555(width, height, frame_dur_us, &frames, lossy, audio, "name", &mut out)?;

// PNG directory → HiColor MVE (with optional WAV):
encode_from_assets_rgb555(&png_paths, wav_path.as_deref(), &opts, &out_dir)?;
```

### Two-stream frame layout

`OC_VIDEO_DATA` payload structure (after the 14-byte header skipped
by the decoder):

```
[u16 LE motion_offset] [colour stream] [motion stream]
```

The colour stream carries opcode 0x5/0x7..0xf payloads; the motion
stream carries 0x2/0x3/0x4. The encoder builds them in two `Vec<u8>`
buffers and writes `motion_offset = 2 + colour_stream.len()`.

### Bit-15 sub-mode selectors

The 16-bit version is structurally **simpler** than the 8-bit one
because the 8-bit "palette ordering" hacks (`pick_pair_descending`,
fabricated phantom palette entries, etc.) are replaced by direct
manipulation of bit 15 of the first u16(s). For example:

- `0x7`: bit 15 of `p[0]` set → per-2×2; clear → per-row
- `0x9`: bit 15 of `p[0]` and `p[2]` choose between four sub-modes
- `0x8`: bit 15 of `p[0]` set → half-split (then bit 15 of `p[2]`
  picks vertical vs horizontal); clear → per-quadrant
- `0xa`: same scheme using `p[0]` and `p[4]`

The decoder strips bit 15 from each marker position after reading,
so any source colour at those positions must have bit 15 = 0 to
keep the round-trip lossless. The chooser checks
`block_has_bit15()` and falls through to the bit-15-safe modes
(0x0/0xe/0x4/0xd/0xc/0xb) if any pixel in the block has bit 15 set.
Verified by `bit15_set_pixel_falls_back_to_lossless_modes`.

### Validation harness

- `tests/rgb555_round_trip.rs` — 24 tests covering each sub-mode
  (`mode_0x7_*`, `mode_0x8_*`, `mode_0x9_*`, `mode_0xa_*`), chooser
  cost-ordering, bit-15 safety, and the OC_VIDEO_MODE / format_flag
  bytes (`init_chunk_emits_hicolor_signals`).
- Cross-checked against gemrb's `ipvideo_decode_frame16` via
  `tools/gemrb_mve_validator/` — 30/30 RGB555 frames decoded clean.

### Mode-distribution evidence on a non-trivial test pattern

A 320×240, 15-frame gradient + moving-square video produces (50 KB
total, comparable to mcomp's HiColor output):

```
0x0 copy_prev_block    91.78%
0x9 quad_b              3.53%
0x7 delta_pattern       1.47%
0xa quad_c              1.21%
0xe solid_colour        0.77%
0xb quad_d              0.70%
0x8 quad_a              0.51%
0xd 8x4_fill            0.03%
```

Every implemented mode fires.

## Phase 11 — built-in palette generation

**Goal**: accept true-colour input (RGB frames) without requiring the
caller to pre-quantise via ffmpeg `palettegen`.

**Where**: a new function `encode_video_truecolour` that takes
`Vec<RgbImage>` instead of palette indices; calls a quantiser to
produce a 256-entry palette, then walks each frame mapping pixels.

**Approach options** (ordered by complexity):
1. Wrap `imagequant` (libimagequant Rust binding). High quality,
   adds a dep.
2. Median-cut, hand-rolled. ~200 lines of Rust. Decent quality.
3. Histogram + reservoir (collect first-seen colours up to 256, then
   approximate the rest). Cheap; only good for pre-quantised input.

**Validation**: write a test that takes a true-colour image with
≤ 256 unique colours, encodes it via the new function, decodes,
and verifies bit-exact round-trip (since no quantisation needed).
Then a second test with a true-colour gradient where lossy
quantisation is unavoidable — verify "close enough" by SSIM or
average-pixel-distance.

**Estimated cost**: 1 session for option 3, 2–3 for option 2,
≤ 1 for option 1 (mostly dep-wrangling).

## Phase 12 — game-engine compatibility validation — **DONE (gemrb)**

**Goal**: confirm files we produce play in real engines.

### What landed

A standalone cross-validator at `tools/gemrb_mve_validator/`
compiles **gemrb's actual decoder source** (`mvevideodec8.cpp`,
`mvevideodec16.cpp`, `mveaudiodec.cpp` — the ffmpeg / Mike Melanson
port that gemrb ships) against stub headers, walks our encoder's
chunk stream, and feeds every frame through gemrb's
`ipvideo_decode_frame{8,16}` and every audio segment through
`ipaudio_uncompress`. Run:

```sh
tools/gemrb_mve_validator/build_and_run.sh
```

Pass extra .mve paths as additional args to validate them too. Defaults
to every `target/mve_encoder/*.mve` from the encoder integration tests.

### Results across the fixture set

- **414 paletted frames** through gemrb's `ipvideo_decode_frame8`:
  0 errors, 0 warnings.
- **30 RGB555 frames** through gemrb's `ipvideo_decode_frame16`:
  0 errors, 0 warnings (every implemented HiColor mode exercised).
- **414 audio segments** through gemrb's `ipaudio_uncompress`:
  no crashes, no out-of-bounds reads.
- **3 of 3 known-good real reference MVEs** (avi2mve smptebars,
  mcomp PalColor, mcomp HiColor) also pass — confirming the harness
  is calibrated.
- **1 of 1 known-bad placeholder** (`gemrb/override/pst/cannon.mve`,
  100-byte deliberately-malformed override stub) correctly FAILS —
  confirming the harness rejects bad input.

### Issues caught during validation (and fixed in earlier sessions)

- Our `OC_VIDEO_MODE` flags were `0x0000`. Both `mcomp.exe` and
  `avi2mve` emit `0x0101` (PalColor) or `0x0110` (HiColor); now fixed.
- gemrb's signature check at first appears strict (full 26-byte
  compare against `"Interplay MVE File\x1A\x00\x1A\x00\x00\x01\x11\x33"`)
  but `FixedSizeString::operator==` uses `strncmp`-with-`length()`,
  both of which stop at the NUL byte at offset 19. Effectively
  permissive; bytes 24-25 can be anything (we write `0x33 0x11` per
  the avi2mve / mcomp / PS:T convention).

### Still un-checked

- **NearInfinity** — Java-based editor/viewer; has an MVE preview
  tab. Drop a file in and click play. Worth a quick sanity check at
  some point, but gemrb's decoder is the relevant production target.
- **Real game** (BG2, PST) — replace one cutscene MVE in the BIF
  archive with one of ours and launch the relevant scene. The bif
  importer in this repo can do the swap; the only blockers are
  acquiring the game data and running the engine. Not yet attempted.

### Concerns from the original phase plan that turned out to be non-issues

- `min_buf` audio hint of `0x10000` — gemrb reads it as a u32 and
  uses it to size the audio queue. Our value works; no stutter
  reported.
- `seq` numbers starting at 0 — gemrb does not validate them.
- Audio sample-rate 22050 — gemrb plays it back without resampling.

## Phase priority recommendation

Phases **6, 9, 10, 12 (gemrb), and the HiColor parity items (lossy
fallback + `encode_from_assets_rgb555`) are DONE**. Remaining
priority order:

### Real work left

| Phase | What | Why |
|---|---|---|
| **7** | `0x5` extended motion (±128 px window) | Modest size win on real cutscenes with camera pans > 8 px. Applies to both Palette8 and RGB555 paths. Brute-force = 65 280 candidates per block; needs a hash index for performance. |
| **11** | Built-in palette generation for true-colour input | Removes the `ffmpeg palettegen` pre-step. `imagequant`, hand-rolled median-cut, or histogram + reservoir — see options in the Phase 11 section. |
| **12 cont.** | NearInfinity preview / real-game cutscene swap-in | gemrb is the load-bearing target and passes; these are belt-and-braces. |

### Defer

- Phase 8 (`0x1`/`0x2`/`0x3` temporal modes) — avi2mve emits them at
  < 0.05 % even on noise. Only relevant if chasing byte-exact
  reproduction of avi2mve output.
- Lossy `0x7`/`0x9` paths — would close the remaining size gap on
  noise/mandelbrot fixtures vs avi2mve, but introduces a new lossy
  axis we don't currently expose.
- Misc cosmetic segments (`OP 0x15` banner trailer, `OP 0x13`/`0x14`
  per-frame trailers) — confirmed unused by every reference decoder
  including gemrb.

### Pick-list

If you have **one session**: **Phase 7** (`0x5` extended motion).
Highest expected size win on real-world content where camera pans
are common. Brute-force implementation is one session; a hash-index
optimised version is another two.

If you have **a week**: 7 → 11 (palette generation) → close the
HiColor parity gaps (lossy downsample, from_assets). Phase 8 only
if byte-exact reproduction is suddenly required.

## Cross-references

- Per-block decoder code: `src/codecs/mve_decoder/src/video.rs`
- Audio-decoder branches: `src/codecs/mve_decoder/src/decoder.rs:415-490`
- Reference output histograms: run
  `tools/PS gui v3.04/PS gui (files)/mve_test/run_histograms.sh`
- Mode-distribution truth table from avi2mve: see the noise file
  result in [§ Phase 6 Evidence] above (47% `0x8`, etc.).
- Cross-validator harness: `tools/gemrb_mve_validator/`
  (`./build_and_run.sh`).
- Official Interplay MVE encoder (`mcomp.exe` + `mmap.exe`) lives at
  `tools/MVETools_by Interplay/`; runs under DOSBox. PalColor config
  produces `flags=0x0101`; HiColor config produces `flags=0x0110`.
