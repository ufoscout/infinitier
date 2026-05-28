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
`infinitier_bif_resource`, etc.) and then playable by
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
