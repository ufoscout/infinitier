# Infinitier

![Build Status](https://github.com/ufoscout/infinitier/actions/workflows/build_and_test.yml/badge.svg)

A personal project, built just for fun.

The goal is to implement pure Rust readers for all file formats used by the Infinity Engine games:
- Baldur's Gate I & II (standard and Enhanced Editions)
- Planescape: Torment (standard and Enhanced Edition)
- Icewind Dale I & II (standard and Enhanced Editions)

## File Format Status

| Format | Description | Implementation |
|--------|-------------|:--------------:|
| 2DA | Two-dimensional array (data table) | Done |
| ACM | Compressed audio | Done |
| ARE | Area / location data | |
| BAM | Bitmap animation | Done |
| BCS | Compiled script | |
| BIF | Resource archive | Done |
| BMP | Bitmap image | Done |
| CHR | Character record | |
| CHU | UI window and control definitions | |
| CRE | Creature | |
| DLG | Dialogue tree | |
| EFF | Effect | |
| FNT | Font | |
| GAM | Game save state | |
| GLSL | GLSL shader | |
| GUI | GUI definition | |
| IDS | Identifier / enumeration reference | Done |
| INI | Configuration | Done |
| ITM | Item | |
| KEY | Resource index | Done |
| LUA | Lua script | |
| MAZE | Maze data (IWD2) | |
| MENU | WeiDU menu | |
| MOS | Background mosaic image | |
| MUS | Music playlist | |
| MVE | Interplay MVE video | Done |
| PLT | Paperdoll layered bitmap | |
| PNG | PNG image | |
| PRO | Projectile | |
| PVRZ | PVR compressed texture | Done |
| SAV | Save game archive | |
| SQL | SQLite database | |
| SPL | Spell | |
| SRC | Script source | |
| STO | Store / shop | |
| TIS | Tileset | |
| TLK | String table | |
| TOH | String override header (EE) | |
| TOT | String override table (EE) | |
| TTF | TrueType font | |
| VAR | Variables (IWD2) | |
| VEF | Visual effect | |
| VVC | Looping visual component | |
| WAVC | Compressed WAV audio | |
| WAV | Audio | |
| WBM | WebM video | |
| WED | Area geometry and layout | Done |
| WFX | Sound effects configuration | |
| WMAP | World map | |

## AI Usage

A significant portion of the UI code was written with AI assistance because I don't like working on UI code.

These decoders were also translated from C to Rust with AI help:

- **ACM decoder** — translated from [markokr/libacm](https://github.com/markokr/libacm)
- **ACM encoder** — translated from [DLTCEP](https://sourceforge.net/projects/gemrb/files)
- **MVE decoder** — translated from [gemrb/gemrb MVEPlayer](https://github.com/gemrb/gemrb/tree/master/gemrb/plugins/MVEPlayer)


## Resources

- https://gibberlings3.github.io/iesdp/index.htm
- https://github.com/Beamdog/bgfileformats
- https://github.com/Beamdog/bgtools
