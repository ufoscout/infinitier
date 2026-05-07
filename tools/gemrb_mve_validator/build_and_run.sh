#!/usr/bin/env bash
# Cross-validate our MVE encoder output against gemrb's actual decoder
# primitives (`ipvideo_decode_frame8/16` + `ipaudio_uncompress` from
# `gemrb/plugins/MVEPlayer/`). Drops any logging dependency on
# gemrb's full engine — the local `gstmvedemux.h` stub provides
# stderr-based GST_WARNING / GST_ERROR replacements.
#
# Usage:
#   ./build_and_run.sh [path/to/gemrb-source-tree] [extra-mve-files...]
#
# If the gemrb source tree is omitted, defaults to a relative path
# from this repo. The script copies the required gemrb sources into
# the build dir, compiles the validator, and runs it against every
# *.mve produced by the encoder's integration tests, plus any extra
# files passed on the command line.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
INFINITIER_ROOT="$(cd "$HERE/../.." && pwd)"
GEMRB_ROOT="${1:-$INFINITIER_ROOT/../gemrb}"
shift || true

MVE_PLUGIN_SRC="$GEMRB_ROOT/gemrb/plugins/MVEPlayer"
if [[ ! -d "$MVE_PLUGIN_SRC" ]]; then
    echo "error: MVEPlayer source not found at $MVE_PLUGIN_SRC" >&2
    echo "       pass the gemrb source root as the first arg." >&2
    exit 2
fi

BUILD_DIR="$HERE/build"
mkdir -p "$BUILD_DIR"
cp "$MVE_PLUGIN_SRC/mvevideodec8.cpp"  "$BUILD_DIR/"
cp "$MVE_PLUGIN_SRC/mvevideodec16.cpp" "$BUILD_DIR/"
cp "$MVE_PLUGIN_SRC/mveaudiodec.cpp"   "$BUILD_DIR/"
cp "$HERE/main.cpp"                    "$BUILD_DIR/"
cp "$HERE/gstmvedemux.h"               "$BUILD_DIR/"

g++ -std=c++17 -O2 -Wno-format \
    -o "$BUILD_DIR/validator" \
    "$BUILD_DIR/main.cpp" \
    "$BUILD_DIR/mvevideodec8.cpp" \
    "$BUILD_DIR/mvevideodec16.cpp" \
    "$BUILD_DIR/mveaudiodec.cpp"

# Default fixtures: everything in target/mve_encoder/ from a recent
# `cargo test --release -p infinitier_mve_encoder` run.
FIXTURES=("$INFINITIER_ROOT"/target/mve_encoder/*.mve)
"$BUILD_DIR/validator" "${FIXTURES[@]}" "$@"
