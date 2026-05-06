# infinitier_acm_encoder

Pure-Rust encoder for the Interplay ACM audio format used by Baldur's
Gate I & II, Planescape: Torment, and Icewind Dale I & II.

The format's open-source reference is DLTCEP's `snd2acm` /
`subband.cpp` / `packer.cpp` (Abel Cheung / TeamX, GPL); this crate is
a faithful Rust port of that pipeline alongside a couple of simpler
encoders for testing.

## Three encoder paths

The crate exposes three entry points, in order of increasing
sophistication / decreasing file size:

| Function | Transform | Quantizer | Filler books | Compression | Lossless? |
|---|---|---|---|---|---|
| [`encode_pcm`] | none (`acm_level = 0`) | `pwr=15`, `val=1` | `f_linear` `ind=16` only | ~0× (16 bits/sample) | yes |
| [`encode_pcm_packed`] | none (`acm_level = 0`) | GCD | full per-column book selection | typically 0.7–0.9× | yes |
| [`encode_pcm_subband`] | forward subband / lifting filter | GCD | full per-column book selection | typically 0.3–0.5× | near-lossless* |

`*` The subband transform uses double-precision floats internally and
truncates coefficients to `i16`, while the decoder applies an integer
inverse — the round-trip therefore introduces a small amount of
filter-rounding noise (a few percent of the i16 range, RMS in the low
hundreds for typical content). `acm_level = 0` skips the transform and
is fully bit-exact.

## Usage

### Lossless encoding (no transform, no compression)

```rust,no_run
use infinitier_acm_encoder::encode_pcm;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples: Vec<i16> = vec![0, 1234, -1234 /* ... */];
    let mut out = std::fs::File::create("out.acm")?;
    encode_pcm(&samples, 1, 22050, &mut out)?;
    Ok(())
}
```

### Lossless with the per-column packer

```rust,no_run
use infinitier_acm_encoder::encode_pcm_packed;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples: Vec<i16> = vec![0; 1024];
    let mut out = std::fs::File::create("silence.acm")?;
    encode_pcm_packed(&samples, 1, 22050, &mut out)?;
    Ok(())
}
```

### Full pipeline — subband transform + packer

```rust,no_run
use infinitier_acm_encoder::encode_pcm_subband;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples: Vec<i16> = vec![0; 22050];
    let mut out = std::fs::File::create("out.acm")?;
    // acm_level = 7 (128 columns/block), acm_rows = 16 — DLTCEP defaults.
    encode_pcm_subband(&samples, 1, 22050, 7, 16, &mut out)?;
    Ok(())
}
```

### Encode an existing WAV file

```rust,no_run
use infinitier_acm_encoder::{encode_wav, encode_wav_subband};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::fs::File::open("input.wav")?;
    let mut output = std::fs::File::create("output.acm")?;
    // Lossless v1:
    // encode_wav(&mut input, &mut output)?;
    // Compressed:
    encode_wav_subband(input, &mut output)?;
    Ok(())
}
```

Only 16-bit signed-integer PCM input is supported (matching the format
the decoder produces).
