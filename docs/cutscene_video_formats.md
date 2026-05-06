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
  src/lib.rs              — bitstream emitter + per-block chooser
  src/from_assets.rs      — high-level encode_from_assets API
  examples/encode_avi.rs  — CLI: ffmpeg → encode_video
  examples/build_test_assets.sh  → tools/build_mve_encoder_assets.sh
  tests/round_trip_it.rs           — synthetic per-mode tests (Phase 1-5)
  tests/from_assets_round_trip.rs  — encode/decode every asset folder
src/codecs/mve_decoder/
  src/decoder.rs          — chunk + segment loop, audio buffers
  src/video.rs            — every decode8_0xN / decode16_0xN function
  examples/block_mode_histogram.rs  — `cargo run --example block_mode_histogram <file.mve>`
tools/
  build_mve_encoder_assets.sh   — populates assets/mve_encoder/<name>/
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
| Palette-8 video, all chooser modes 0x0/0x4/0x7/0x9/0xb/0xc/0xd/0xe | `lib.rs::encode_block` |
| Lossy `0xc` fallback when raw would overflow segment cap | `lib.rs::build_4x4_fill_downsampled` |
| 16×16 brute-force motion search | `lib.rs::find_motion_match` |
| Multi-frame skip detection via `VIDEO_FLAG_DELTA` swap | `lib.rs::encode_av` |
| Uncompressed 16-bit-PCM audio (mono + stereo) | `lib.rs::build_init_audio_chunk`, `build_frame_chunk` |
| `encode_from_assets` (PNG dir + WAV → .mve) | `from_assets.rs` |
| Round-trip integration tests across 10 fixtures | `tests/from_assets_round_trip.rs` |

The encoder is **lossless on any palette-8 input** that fits the
65 535-byte segment cap; high-detail content needs `lossy_downsample`
(currently a per-block 2×2 top-left pick).

## Phase 6 — alternative quad-pattern modes 0x8 + 0xa

**Goal**: emit modes `0x8` and `0xa` for natural-image content where
they beat our current `0x9` per-pixel (20 bytes) + `0xb` raw (64
bytes) fallbacks.

**Evidence it matters**: avi2mve's noise reference uses **47% `0x8`**
+ 5% `0xa`. Our equivalent `from_assets` output uses 0% of either; we
fall through to lossy `0xc` (16 bytes) instead. Result: avi2mve's
noise.mve is significantly smaller than ours.

**Decoder reference**: `mve_decoder/src/video.rs`

- `decode8_0x8` (lines ~174–284): two sub-modes selected by
  `p[0] <= p[1]`. The "yes" branch reads 4×4 quadrants × 2 colours
  each (16 bytes total); the "no" branch reads 2 halves
  (vertical or horizontal) with 2 colours per half.
  See `pack_flags_8` (lines ~287–296) for the bit layout.
- `decode8_0xa` (lines ~381–462): four-colour-per-quadrant or
  four-colour-per-half with 16-bit masks. Even more sub-modes than
  `0x8`.

**Where to wire it in**: `lib.rs::encode_block`, between the existing
`0x7` (delta) and `0x9` (quad-pattern) fallback levels — these modes
are 2-colour-per-quadrant and 4-colour-per-half so they sit between
the 4-byte 0x7 and the 8/12/20-byte 0x9. Likely insertion order:

```
… → 0x7 → if can_encode_0x8 → 0x8 → 0x9 → 0xa → …
```

**Algorithm sketch (mode 0x8 "p0 ≤ p1" branch)**:
1. For each 4×4 quadrant of the 8×8 block, count distinct colours.
   Reject if any quadrant has >2 colours.
2. Order the 8 distinct palette indices into `p[0..8]` so `p[0] ≤ p[1]`.
3. Build 8 byte-masks `b[0..8]` matching `pack_flags_8`'s read order.

**Algorithm sketch (mode 0xa "p0 ≤ p1" branch)**:
1. Each quadrant has up to 4 colours.
2. Build `p[0..16]` (4 colours × 4 quadrants).
3. Build `b[0..16]` for the per-pixel index masks.

**Validation**:
1. Add per-mode round-trip tests in `tests/round_trip_it.rs` matching
   the existing `quad_pattern_*` style.
2. Re-run `from_assets_round_trip`: expect noise.mve to shrink and
   *probably* drop the `lossy_downsample` flag (since 0x8/0xa cover
   noise content losslessly within the segment cap).
3. Cross-check histograms: ours should show non-zero `0x8` and `0xa`
   counts on natural content like `mandelbrot` and `noise`.

**Estimated cost**: 1–2 sessions. The decoder's `pack_flags_8` is
fiddly — encode the mask bits in the exact reverse order the decoder
unpacks them, and add a unit test that round-trips a single block in
isolation before attempting full-frame.

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

## Phase 9 — Interplay DPCM compressed audio

**Goal**: emit `AUDIO_FLAG_COMPRESSED` audio in the **Interplay DPCM**
format that avi2mve.exe produces. ~50% smaller than raw PCM (1 byte
per sample after the seed instead of 2). This is the codec ffprobe
reports as `interplay_dpcm` on real game cutscenes and on avi2mve
output; we currently emit `pcm_s16le` — interoperable but visibly
different in `ffprobe -show_streams`.

**Format spec** (already implemented on the decode side):

- Stream header: one i16 LE **predictor seed per channel**
  (1 word for mono, 2 words for stereo, written L then R).
- Then a stream of 1-byte deltas. Each byte indexes into a 256-entry
  signed-i16 lookup table. The decoder adds the table value to the
  current channel's predictor, saturates to `[-32768, 32767]`,
  emits the saturated result, and switches channel (stereo only).

The lookup table itself is `DELTA_TABLE` in
`src/codecs/mve_decoder/src/audio.rs:4-20`. The decoder is
`decompress_audio` at `audio.rs:29-60`. The encoder must mirror it
byte-for-byte to avoid predictor drift.

**Cross-reference for the encoder side**:
FFmpeg has a working Interplay DPCM encoder at
`libavcodec/interplay_dpcm.c` in the FFmpeg tree (search
`AVCodec ff_interplay_dpcm_encoder`). It's reasonable to read that
as the reference algorithm — it also uses the same lookup table and
the same saturation rules.

**Where to edit**:

- `src/codecs/mve_encoder/src/lib.rs::build_init_audio_chunk`:
  set `AUDIO_FLAG_COMPRESSED` (0x0004) in the flags word and bump
  the segment's version byte from 1 to 2 (the decoder only honours
  the COMPRESSED flag when `version > 0`; check
  `decoder.rs::read_audio_buffers`).
- `src/codecs/mve_encoder/src/lib.rs::build_frame_chunk`: replace
  the raw `extend_from_slice(&s.to_le_bytes())` loop with the
  compressor (see algorithm below). The 6-byte header
  (seq, stream_mask, audio_size) stays the same; `audio_size`
  remains the *uncompressed* sample-byte count
  (i.e. `n_samples * 2`) because that's what the decoder uses to
  size its output buffer (`audio.rs:30`: `total_samples = audio_size / 2`).
- Make `DELTA_TABLE` reachable from the encoder. Either expose it
  as `pub const` from `mve_decoder::audio` and add a dependency
  edge (encoder doesn't currently depend on decoder — adding the
  edge is fine since the workspace already has the inverse
  relationship as a `dev-dependency`), or copy the 256 entries into
  the encoder. Copying is simpler if we don't want to add the dep.

**Algorithm — compressor**:

The first sample of each channel is the predictor seed (write i16 LE
verbatim). For every subsequent sample, **find the delta-byte
whose post-saturation predictor is closest to the target sample**:

```text
for each subsequent sample s (channel-interleaved):
    best_byte = 0
    best_dist = i32::MAX
    for b in 0..256:
        candidate = (predictor[ch] + DELTA_TABLE[b] as i32).clamp(-32768, 32767)
        dist = (candidate - s as i32).abs()
        if dist < best_dist:
            best_byte = b
            best_dist = dist
    write best_byte
    predictor[ch] = (predictor[ch] + DELTA_TABLE[best_byte] as i32).clamp(-32768, 32767)
    ch ^= channels - 1
```

The 256-entry inner loop is fine for offline encoding. Optimisation
later: precompute a sorted index over the table, binary-search for
the closest delta. Watch out — the table is **non-monotonic**: see
indices 124–128 (`-1, 1, 1, 5481, -32589`) for the discontinuity
where the encoding flips from "small positive" to "small negative".
A naïve binary search will miss matches; either search both halves
or fall back to linear when the linear distance is small.

**Validation**:

1. Add a unit test in `from_assets.rs` (or `lib.rs`): encode a known
   i16 sequence, decode it via `decompress_audio`, assert each
   reconstructed sample is within ±1 LSB of the source.
2. Extend `tests/from_assets_round_trip.rs` to optionally encode
   with `audio_compressed: true`. The current sample-exact
   comparison must be relaxed: assert mean absolute error ≤ 8 LSB,
   max absolute error ≤ 64 LSB, and total length matches exactly.
3. ffprobe sanity check: `ffprobe -show_streams target/mve_encoder/<file>.mve`
   should now report `codec_name=interplay_dpcm` for the audio
   stream.

**Estimated cost**: 1–2 sessions. The compressor itself is ~50 lines
once the decoder reference is open; the test relaxation and the
optional binary-search optimisation are the slow parts.

## Phase 10 — RGB555 (16-bit) video

**Goal**: encode 16-bit-per-pixel RGB555 cutscenes (some Infinity
Engine games ship a few of these — confirm by grepping
`format_flag != 0` in `decode_frame16`).

**Decoder reference**: `mve_decoder/src/video.rs::decode_frame16`
(lines ~1152+) plus a parallel set of `decode16_0xN` per-block
functions. Most modes have 16-bit equivalents that operate on
`u16` pixels with `BPP16 = 2`.

**Where**:
- `OC_VIDEO_BUFFERS` payload: set `format_flag = 1` and bump segment
  version to ≥ 2.
- Every `encode_block` mode needs a 16-bit cousin: 0x4 motion search
  on u16 buffers, 0x7/0x9/0xc/0xb working on u16 instead of u8 (no
  palette).
- `OC_PALETTE` segment is irrelevant in RGB555 mode — drop from init.

**When this matters**: only if we want to ingest true-colour AVIs
without quantising. For the test fixtures everything is palette-8,
so this is purely additive.

**Estimated cost**: 3–5 sessions. It's largely a parallel
implementation; the per-block payload formats are similar but every
size constant doubles.

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

## Phase 12 — game-engine compatibility validation

**Goal**: confirm files we produce play in real engines.

**Targets, in order of accessibility**:
1. **gemrb** — open source Infinity Engine clone with built-in MVE
   player. Build and play one of our test outputs:
   ```sh
   gemrb --play-movie target/mve_encoder/320x240_15fps_3s_smptebars.mve
   ```
2. **NearInfinity** — Java-based editor/viewer; has an MVE preview
   tab. Drop a file in and click play.
3. **Real game** (BG2, PST) — replace one cutscene MVE in the BIF
   archive with one of ours and launch the relevant scene.

**What may go wrong**:
- Our `OC_VIDEO_MODE` segment hard-codes `640x480`. Real games may
  parse this and refuse non-game-resolution clips. Look at avi2mve's
  output to see what it does for sub-640 content.
- Our `min_buf` audio hint is `0x10000`. Some decoders compute
  buffer sizes from this; mismatched values may cause stutter.
- `seq` numbers we write start at 0; avi2mve starts at 1 in some
  places. Probably tolerated but worth checking.
- Audio sample-rate of 22050 is conservative; real cutscenes often
  use 11025 or 8000 to fit chunk-size budgets at low frame rates.

**Estimated cost**: 1 session for gemrb. Add bug-fix sessions as
issues surface.

## Phase priority recommendation

If you have one session: **Phase 6** (modes 0x8 / 0xa). It's the
biggest size win on real content and gets us closer to byte-similar
output to avi2mve.

If you have a week: 6 → 9 (compressed audio, halves audio bytes) →
12 (validate against gemrb).

Defer until needed:
- Phase 7 (`0x5` extended motion) — niche.
- Phase 8 (`0x1`/`0x2`/`0x3`) — diminishing returns.
- Phase 10 (RGB555) — only if a use case for true-colour MVE exists.
- Phase 11 (palette generation) — `encode_from_assets` already
  handles this implicitly via the asset pipeline.

## Cross-references

- Per-block decoder code: `src/codecs/mve_decoder/src/video.rs`
- Audio-decoder branches: `src/codecs/mve_decoder/src/decoder.rs:415-490`
- Reference output histograms: run
  `tools/PS gui v3.04/PS gui (files)/mve_test/run_histograms.sh`
- Mode-distribution truth table from avi2mve: see the noise file
  result in [§ Phase 6 Evidence] above (47% `0x8`, etc.).
