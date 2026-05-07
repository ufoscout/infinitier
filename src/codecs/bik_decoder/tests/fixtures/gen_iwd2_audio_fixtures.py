#!/usr/bin/env python3
"""
Generate IWD2 audio fixtures: one raw 16-bit PCM file per .mve, plus a
manifest mapping the relative path to the sample count and a SHA-256 of
the raw PCM bytes.

Output layout (under tests/fixtures/):
    iwd2_audio.txt              # manifest: per-file sample count + sha256
    iwd2_audio/<name>.s16le     # raw little-endian 16-bit signed PCM,
                                # interleaved if stereo

The Rust corpus test decodes the audio, compares byte-exactly when
possible, and falls back to PSNR otherwise (DCT float precision can drift
between implementations even with otherwise-correct code).

Usage:
    python3 gen_iwd2_audio_fixtures.py [iwd2_root]
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from pathlib import Path

FILES = [
    ("Data/BISlogo.mve", 22050, 2),
    ("Data/Credits.mve", 22050, 2),
    ("Data/Nvidia.mve", 48000, 2),
    ("Data/WOTC.mve", 22050, 2),
    ("CD2/Data/END.mve", 22050, 2),
    ("CD2/Data/Intro.mve", 22050, 2),
    ("CD2/Data/Middle.mve", 44100, 2),
]


def dump_pcm(src: Path, dest: Path, sample_rate: int, channels: int) -> None:
    """Run ffmpeg to write raw s16le PCM."""
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-loglevel", "error",
            "-i", str(src),
            "-vn",
            "-f", "s16le",
            "-acodec", "pcm_s16le",
            "-ar", str(sample_rate),
            "-ac", str(channels),
            str(dest),
        ],
        check=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "iwd2_root",
        nargs="?",
        default="/home/ufo/Temp/Games/Icewind Dale 2",
        type=Path,
    )
    args = parser.parse_args()

    here = Path(__file__).resolve().parent
    pcm_dir = here / "iwd2_audio"
    pcm_dir.mkdir(exist_ok=True)
    manifest_path = here / "iwd2_audio.txt"

    lines: list[str] = []
    lines.append("# IWD2 cutscene audio fixtures (raw s16le PCM)")
    lines.append("# Per-line: <relative path> <sample_rate> <channels> <byte_count> <sha256>")
    for rel, rate, ch in FILES:
        src = args.iwd2_root / rel
        if not src.is_file():
            print(f"!! missing {src}; skipping", file=sys.stderr)
            continue
        dest = pcm_dir / (Path(rel).stem + ".s16le")
        print(f"dumping {rel} → {dest.name}", file=sys.stderr)
        dump_pcm(src, dest, rate, ch)
        data = dest.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        lines.append(f"{rel} {rate} {ch} {len(data)} {digest}")
        print(f"  {len(data)} bytes ({len(data) // (2 * ch)} samples)", file=sys.stderr)

    manifest_path.write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
