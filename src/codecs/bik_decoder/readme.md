# infinitier_bik_decoder

Pure-Rust decoder for **Bink Video v1** (`BIKi`), the format the IWD2 cutscenes
ship under (despite their `.mve` extension).

The implementation is a port of FFmpeg's `libavcodec/bink.c` and
`binkaudio.c` (release/6.1 snapshot), with all the post-2009 bug fixes
that GemRB's BIKPlayer never picked up.

Supported variants in scope:

| Codec | What it covers |
|---|---|
| `binkvideo` (BIKi/BIKb/BIKf) | Bink Video v1 with YUV420p output, optional alpha |
| `binkaudio_dct` | DCT-based Bink audio (the variant IWD2 uses) |

`binkvideo2` (KB2) and `binkaudio_rdft` are out of scope.

See `tests/iwd2_corpus.rs` for the per-frame SHA-256 fixture validation
against the seven IWD2 cutscene files.
