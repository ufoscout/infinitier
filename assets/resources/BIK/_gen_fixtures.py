#!/usr/bin/env python3
"""
Generate per-file JSON fixtures for the Bink corpus under
`assets/resources/BIK/`.

For each `.bik` / `.mve` file:
* probes container metadata via ffprobe;
* dumps every video frame as YUV420p (or alpha-padded YUVA420p when
  applicable) and hashes each frame independently;
* dumps the audio track (if any) as raw `s16le` PCM and hashes the whole
  stream;
* writes a sibling `<stem>.json` with the same shape used by the MVE
  fixtures (`assets/resources/MVE/16_bits/BISLOGO.json`), adapted for
  Bink (`codec_tag` replaces the palette bit-depth).

Re-running is idempotent — JSONs are overwritten in full.

Usage:
    python3 _gen_fixtures.py
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Single source of truth for which extensions count as Bink. The corpus
# mixes `.bik` (lowercase), `.BIK` (uppercase) and `.mve` (IWD2-style
# disguised Bink).
BINK_EXTS = {".bik", ".BIK", ".mve", ".MVE"}


def ffprobe_json(path: Path) -> dict:
    """Run `ffprobe -show_format -show_streams` and return the parsed JSON."""
    out = subprocess.check_output(
        [
            "ffprobe",
            "-v", "0",
            "-show_format",
            "-show_streams",
            "-print_format", "json",
            str(path),
        ]
    )
    return json.loads(out)


def hash_video_frames(path: Path, width: int, height: int) -> list[str]:
    """Pipe the file's video as YUV420p; return SHA-256 hex per frame."""
    frame_size = width * height + 2 * (width // 2) * (height // 2)
    proc = subprocess.Popen(
        [
            "ffmpeg",
            "-loglevel", "error",
            "-i", str(path),
            "-an",  # explicitly drop audio in case the file has both
            "-f", "rawvideo",
            "-pix_fmt", "yuv420p",
            "-",
        ],
        stdout=subprocess.PIPE,
    )
    assert proc.stdout is not None
    hashes: list[str] = []
    buf = b""
    while True:
        chunk = proc.stdout.read(frame_size - len(buf))
        if not chunk:
            break
        buf += chunk
        while len(buf) >= frame_size:
            frame, buf = buf[:frame_size], buf[frame_size:]
            hashes.append(hashlib.sha256(frame).hexdigest())
    proc.wait()
    if proc.returncode != 0:
        raise RuntimeError(f"ffmpeg returned {proc.returncode} on {path}")
    if buf:
        raise RuntimeError(f"trailing partial frame ({len(buf)} bytes) for {path}")
    return hashes


def hash_audio_pcm(path: Path, sample_rate: int, channels: int) -> tuple[int, str]:
    """Decode the audio track to raw s16le and return (sample_count, sha256)."""
    proc = subprocess.run(
        [
            "ffmpeg",
            "-loglevel", "error",
            "-i", str(path),
            "-vn",
            "-f", "s16le",
            "-acodec", "pcm_s16le",
            "-ar", str(sample_rate),
            "-ac", str(channels),
            "-",
        ],
        capture_output=True,
        check=True,
    )
    pcm = proc.stdout
    sample_count = len(pcm) // 2  # s16 = 2 bytes per sample (interleaved)
    return sample_count, hashlib.sha256(pcm).hexdigest()


def fixture_for(path: Path) -> dict:
    meta = ffprobe_json(path)
    video_stream = next(s for s in meta["streams"] if s["codec_type"] == "video")
    audio_stream = next(
        (s for s in meta["streams"] if s["codec_type"] == "audio"), None
    )

    width = int(video_stream["width"])
    height = int(video_stream["height"])
    fps_num, fps_den = (int(x) for x in video_stream["r_frame_rate"].split("/"))
    frame_duration_us = (1_000_000 * fps_den) // fps_num
    codec_tag = video_stream.get("codec_tag_string", "")

    print(f"  hashing {path.name} video ({width}x{height}, {codec_tag}) ...",
          file=sys.stderr, flush=True)
    frame_hashes = hash_video_frames(path, width, height)

    fixture: dict = {
        "video": {
            "codec_tag": codec_tag,
            "width": width,
            "height": height,
            "frame_count": len(frame_hashes),
            "frame_duration_us": frame_duration_us,
            "frame_hashes": frame_hashes,
        },
    }

    if audio_stream is not None:
        sample_rate = int(audio_stream["sample_rate"])
        channels = int(audio_stream["channels"])
        print(
            f"  hashing {path.name} audio ({sample_rate} Hz, {channels}ch) ...",
            file=sys.stderr,
            flush=True,
        )
        total_samples, wav_sha = hash_audio_pcm(path, sample_rate, channels)
        layout = "stereo" if channels == 2 else "mono" if channels == 1 else f"{channels}ch"
        fixture["audio"] = {
            "channels": channels,
            "sample_rate": sample_rate,
            "bits_per_sample": 16,
            "format": f"PCM 16-bit {layout} at {sample_rate} Hz",
            "total_samples": total_samples,
            "wav_sha256": wav_sha,
        }
    else:
        fixture["audio"] = None

    return fixture


def main() -> int:
    bink_files = sorted(p for p in HERE.iterdir() if p.suffix in BINK_EXTS)
    if not bink_files:
        print("no .bik / .mve files found", file=sys.stderr)
        return 1
    for src in bink_files:
        print(f"== {src.name} ==", file=sys.stderr)
        fix = fixture_for(src)
        # `<stem>.json` matches the MVE fixture convention.
        out = src.with_suffix(".json")
        out.write_text(json.dumps(fix, indent=2) + "\n")
        print(f"  wrote {out.name}  ({len(fix['video']['frame_hashes'])} frame hashes)",
              file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
