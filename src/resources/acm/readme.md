# infinitier_acm_resource

Importer for `*.ACM` sound resources shipped by Infinity Engine games.

Two flavours live under the `.acm` extension in the wild:

- **Real ACM** — the historical Interplay sound codec. Decoded by
  [`infinitier_acm_decoder`](../../codecs/acm_decoder).
- **OGG/Vorbis** — some Enhanced-Edition sound packs ship `.acm` files
  that are actually OGG/Vorbis streams. The importer detects them by
  the four-byte `OggS` magic and delegates decoding to
  [`infinitier_wav_resource::WavDecoder`](../wav), which already wires up
  Symphonia's Ogg/Vorbis demuxer + decoder for the WAV resource's own
  OGG-under-`.wav` branch.

`AcmImporter::import` returns a single [`Acm`] enum carrying the
appropriate streaming decoder. Inherent helpers (`read_samples`,
`decode_all`, `channels`, `sample_rate`, `total_values`, `reset`) work
uniformly across both branches; drop down to `match` for decoder-
specific knobs.

## Usage

```rust,no_run
use infinitier_acm_resource::{Acm, AcmFormat, AcmImporter};
use infinitier_datasource::{DataSource, Importer};

fn main() -> std::io::Result<()> {
    let source = DataSource::new("VOICE/bc1a1.acm");
    let mut dec: Acm = AcmImporter { name: "bc1a1" }.import(&source)?;

    println!(
        "{:?} stream: {} ch @ {} Hz, {} interleaved i16",
        dec.format(),
        dec.channels(),
        dec.sample_rate(),
        dec.total_values(),
    );

    let mut buf = [0i16; 4096];
    loop {
        let n = dec.read_samples(&mut buf)?;
        if n == 0 { break; }
        // ... feed buf[..n] to the audio backend ...
    }
    Ok(())
}
```
