#!/usr/bin/env bash
# Builds test assets for the MVE encoder integration test.
# For each .avi in $SRC_DIR, produces:
#   $OUT_DIR/<basename>/frame_0001.png ... frame_NNNN.png  (paletted, ≤256 colours)
#   $OUT_DIR/<basename>/audio.wav                          (22050 Hz mono, 16-bit PCM)
#
# If the AVI lacks an audio stream, a silent WAV of matching duration is generated.

set -euo pipefail

SRC_DIR="${1:-/home/ufo/workspaces/github_ufoscout/baldurs_gate/tools/PS gui v3.04/PS gui (files)/mve_test}"
OUT_DIR="${2:-$(dirname "$0")/../assets/mve_encoder}"

mkdir -p "$OUT_DIR"

shopt -s nullglob
for avi in "$SRC_DIR"/*.avi; do
    name="$(basename "$avi" .avi)"
    dest="$OUT_DIR/$name"
    echo ">> $name"
    rm -rf "$dest"
    mkdir -p "$dest"

    # Probe duration (seconds) and audio-stream presence.
    duration=$(ffprobe -v error -select_streams v:0 \
        -show_entries stream=nb_frames,r_frame_rate \
        -of default=noprint_wrappers=1:nokey=1 "$avi" \
        | awk 'NR==1{fps=$0} NR==2{nb=$0}
               END{split(fps,a,"/"); printf "%.6f", nb*a[2]/a[1]}')
    has_audio=$(ffprobe -v error -select_streams a:0 \
        -show_entries stream=codec_type \
        -of default=noprint_wrappers=1:nokey=1 "$avi" || true)

    # Paletted frames: single-pass split → palettegen → paletteuse,
    # written as paletted PNGs (image2 muxer). No dithering, so each
    # output pixel is exactly one of the 256 palette entries.
    ffmpeg -y -loglevel error -i "$avi" \
        -vf 'split[a][b];[a]palettegen=max_colors=256:reserve_transparent=0[p];[b][p]paletteuse=dither=none' \
        "$dest/frame_%04d.png"

    # Audio: extract or synthesise silence at 22050 mono 16-bit PCM.
    if [[ -n "$has_audio" ]]; then
        ffmpeg -y -loglevel error -i "$avi" \
            -vn -ar 22050 -ac 1 -acodec pcm_s16le "$dest/audio.wav"
    else
        ffmpeg -y -loglevel error \
            -f lavfi -i "anullsrc=r=22050:cl=mono" \
            -t "$duration" -acodec pcm_s16le "$dest/audio.wav"
    fi
done

echo "done"
