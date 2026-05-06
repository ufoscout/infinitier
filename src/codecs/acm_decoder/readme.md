# infinitier_acm_decoder

A pure-Rust decoder for the Interplay ACM audio format, used by Baldur's Gate I & II, Planescape: Torment, and Icewind Dale I & II (both standard and Enhanced Editions).

Ported from the C implementation by Marko Kreen ([libacm](https://github.com/markokr/libacm)).

## Supported containers

The decoder accepts two on-disk layouts and autodetects which one it is reading from the magic bytes at the start of the stream:

- **Bare ACM** — the raw Interplay ACM bitstream (`0x97280301` signature). This is what `*.ACM` files in the games' BIFs contain.
- **WAVC** — a 28-byte Interplay header (`'WAVC'`, `'V1.0'`, sizes, ACM-data pointer, channel count, bits per sample, sample rate) wrapped around a regular ACM bitstream. WAVC files are 22050 Hz / 16-bit and ship as `*.WAV` in the games' override folders and BIFs (despite the extension, they are not RIFF/WAVE). `AcmDecoder::open` skips the WAVC header transparently and decodes the embedded ACM payload, so callers do not need to strip it themselves.

For real RIFF/WAVE files (the other flavour shipped under the `*.WAV` extension), use the `infinitier_wav_decoder` crate, which dispatches between the two.

## Usage

### Decode to a WAV file

```rust,no_run
use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("sound.acm");
    let mut decoder = AcmDecoder::open(&source, OutputChannels::Original)?;
    decoder.decode_to_file("sound.wav")?;
    Ok(())
}
```

### Decode to raw PCM samples

```rust,no_run
use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("sound.acm");
    let mut decoder = AcmDecoder::open(&source, OutputChannels::Original)?;

    let info = &decoder.info;
    println!("channels: {}, sample rate: {} Hz", info.channels, info.rate);
    println!("samples: {}", info.samples());

    // Interleaved i16 PCM samples (one per channel per frame)
    let samples: Vec<i16> = decoder.decode_all()?;
    Ok(())
}
```

### Force a specific channel count

Use `OutputChannels::Mono` or `OutputChannels::Stereo` to override the channel
count stored in the file header, for example when a stereo ACM is referenced
from a mono sound entry in the game's KEY/BIF resources.

```rust,no_run
use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("stereo_stored_as_mono.acm");
    let mut decoder = AcmDecoder::open(&source, OutputChannels::Stereo)?;
    decoder.decode_to_file("output.wav")?;
    Ok(())
}
```
