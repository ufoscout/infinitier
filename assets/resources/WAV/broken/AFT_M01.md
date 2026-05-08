# AFT_M01.WAV — inconsistent fmt chunk

This file is shipped by some Infinity Engine assets with an internally
inconsistent `fmt ` chunk. It plays correctly in NearInfinity and gemrb
but is rejected (or silently mis-decoded) by stricter parsers like
`hound` and `symphonia`'s WAV reader. We keep it here as a fixture for
the test that pins our own lenient PCM reader.

## What's wrong

Hex of the first 36 bytes:

```
00000000: 5249 4646 1cc7 0100 5741 5645 666d 7420  RIFF....WAVEfmt 
00000010: 1000 0000 0100 0100 2256 0000 44ac 0000  ........"V..D...
00000020: 0400 1000 6461 7461 ....                 ....data
```

Decoded fmt chunk:

| Field             | Value          | Notes                                   |
|-------------------|----------------|-----------------------------------------|
| `wFormatTag`      | `0x0001` (PCM) | OK                                      |
| `nChannels`       | `1`            | OK                                      |
| `nSamplesPerSec`  | `22050`        | OK                                      |
| `nAvgBytesPerSec` | `44100`        | Consistent with `block_align = 2`       |
| `nBlockAlign`     | `4`            | **Wrong** — should be `1 × 16/8 = 2`    |
| `wBitsPerSample`  | `16`           | OK                                      |
