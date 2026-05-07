# infinitier_bik_decoder

Pure-Rust decoder for **Bink Video v1** (`BIKi`), the format the IWD2 cutscenes
ship under (despite their `.mve` extension).

The implementation is a port of FFmpeg's `libavcodec/bink.c` and
`binkaudio.c` (release/6.1 snapshot).

Supported variants in scope:

| Codec | What it covers |
|---|---|
| `binkvideo` (BIKi/BIKf/BIKg/BIKh/BIKk) | Bink Video v1 with YUV420p output, optional alpha |
| `binkvideo` (BIKb) | BinkB |
| `binkaudio_dct` | DCT-based Bink audio |
| `binkaudio_rdft` | RDFT-based Bink audio |

`binkvideo2` (KB2) is out of scope.
