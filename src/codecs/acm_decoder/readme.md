# infinitier_acm_decoder

A pure-Rust decoder for the Interplay ACM audio format, used by Baldur's Gate I & II, Planescape: Torment, and Icewind Dale I & II (both standard and Enhanced Editions).

Ported from the C implementation by Marko Kreen ([libacm](https://github.com/markokr/libacm)).

## Usage

### Decode to a WAV file

```rust,no_run
use infinitier_acm_decoder::{AcmDecoder, OutputChannels};
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("sound.acm");
    let reader = source.reader()?;
    let mut decoder = AcmDecoder::open(reader, OutputChannels::Original)?;
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
    let mut decoder = AcmDecoder::open(source.reader()?, OutputChannels::Original)?;

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
    let mut decoder = AcmDecoder::open(source.reader()?, OutputChannels::Stereo)?;
    decoder.decode_to_file("output.wav")?;
    Ok(())
}
```
