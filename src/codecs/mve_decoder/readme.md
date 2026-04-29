# infinitier_mve_decoder

A pure-Rust decoder for the Interplay MVE video format, used for cutscenes in Baldur's Gate I & II, Planescape: Torment, and Icewind Dale I & II (both standard and Enhanced Editions).

Translated from the C implementation in [gemrb/gemrb MVEPlayer](https://github.com/gemrb/gemrb/tree/master/gemrb/plugins/MVEPlayer).

## Usage

### Decode frame by frame

```rust,no_run
use infinitier_mve_decoder::MveDecoder;
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("intro.mve");
    let mut decoder = MveDecoder::new(source.reader()?)?;

    println!("{}x{} @ {} µs/frame", decoder.width(), decoder.height(), decoder.frame_duration_us());

    while let Some(frame) = decoder.next_frame()? {
        // frame.video.pixels — RGBA bytes, width * height * 4 bytes
        // frame.video.duration_us — display duration for this frame
        // frame.audio — Vec<AudioChunk> with interleaved i16 PCM samples
        let _rgba: &[u8] = &frame.video.pixels;
    }
    Ok(())
}
```

### Extract the audio track to WAV

```rust,no_run
use infinitier_mve_decoder::MveDecoder;
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("intro.mve");
    let decoder = MveDecoder::new(source.reader()?)?;
    decoder.extract_audio_to_wav("intro_audio.wav")?;
    Ok(())
}
```
