# infinitier_acm_encoder

Pure-Rust encoder for the Interplay ACM audio format used by Baldur's Gate I & II, Planescape: Torment, and Icewind Dale I & II.

## Status: minimal lossless v1

The Interplay ACM bitstream allows several quantization powers, several
filler "books" (Huffman / variable-length / ternary / quinary
distributions), and a multi-octave wavelet-style lifting transform.
Picking the optimal combination per block is what gives the format its
compression ratio.

This crate currently implements the **trivial corner case** of the
bitstream: `acm_level = 0` (no lifting transform), `pwr = 15` / `val = 1`
(amplitude lookup table is the identity over the full i16 range), and a
single column per block encoded with the `f_linear` filler at `ind = 16`
(16 bits per sample). The result is a valid, fully decodable ACM file
that round-trips PCM samples bit-for-bit through
`infinitier_acm_decoder`.

The compression ratio is therefore ~0% — every i16 sample uses 16 bits
plus a tiny per-block header — but the output is real ACM, not a custom
shim. Adding the forward lifting transform and the higher-order filler
books to actually compress is future work; the bitstream framing,
header, and round-trip plumbing are already in place.

## Usage

### Encode raw PCM samples

```rust,no_run
use infinitier_acm_encoder::encode_pcm;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let samples: Vec<i16> = vec![0, 1234, -1234 /* ... */];
    let mut out = std::fs::File::create("out.acm")?;
    encode_pcm(&samples, 1, 22050, &mut out)?;
    Ok(())
}
```

### Encode an existing WAV file

```rust,no_run
use infinitier_acm_encoder::encode_wav;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = std::fs::File::open("input.wav")?;
    let mut output = std::fs::File::create("output.acm")?;
    encode_wav(&mut input, &mut output)?;
    Ok(())
}
```

Only 16-bit signed-integer PCM input is supported (matching the format
the decoder produces).
