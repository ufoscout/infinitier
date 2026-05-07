# infinitier_bik_decoder

Pure-Rust decoder for **Bink Video v1** (`BIKi`), the format the IWD2 cutscenes
ship under (despite their `.mve` extension).

The implementation is a port of FFmpeg's `libavcodec/bink.c` and
`binkaudio.c` (release/6.1 snapshot), with all the post-2009 bug fixes
that GemRB's BIKPlayer never picked up.

Supported variants in scope:

| Codec | What it covers |
|---|---|
| `binkvideo` (BIKi/BIKf/BIKg/BIKh) | Bink Video v1 with YUV420p output, optional alpha |
| `binkaudio_dct` | DCT-based Bink audio (the variant IWD2 uses) |

`BIKb` (BinkB) headers parse but its frame decoder isn't implemented yet
— the corpus tests skip it cleanly. `binkvideo2` (KB2) and
`binkaudio_rdft` are out of scope.

## Test corpus

Tests live under `tests/`:

* `tests/container.rs` — header / frame-index / audio-track parsing.
* `tests/video_corpus.rs` — per-frame SHA-256 of the YUV420p output
  against the FFmpeg-recorded fixtures, byte-exact.
* `tests/audio_corpus.rs` — full PCM stream against either the recorded
  SHA-256 (byte-exact) or live FFmpeg re-decode (PSNR ≥ 30 dB).

Fixtures live in `assets/resources/BIK/<stem>.json` next to each
`.bik` / `.mve` file. To regenerate them after adding new files:

```sh
python3 assets/resources/BIK/_gen_fixtures.py
```
