# infinitier_wbm_decoder

Pure-Rust decoder for the **WBM** cutscene format shipped in the Beamdog
Enhanced Editions of Baldur's Gate, Icewind Dale, and Planescape:
Torment.

`.wbm` is just a renamed WebM file: an EBML/Matroska container holding
a VP8 video track and a Vorbis audio track. The crate is a thin glue
layer over three pure-Rust dependencies:

| Layer | Crate |
|---|---|
| EBML / Matroska / WebM demux | `matroska-demuxer` |
| VP8 video decode | `oxideav-vp8` |
| Vorbis audio decode | `lewton` |

No FFI, no `libvpx`, no `libvorbis`.

## Usage

```rust,no_run
use std::fs::File;
use infinitier_wbm_decoder::{WbmOutputFormat, WbmPixels, WbmStreamingDecoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let f = File::open("logo.wbm")?;
    let mut decoder = WbmStreamingDecoder::new(f, "logo.wbm")?
        .with_output_format(WbmOutputFormat::Rgba);

    println!(
        "{}x{} @ {:.2} fps",
        decoder.width(),
        decoder.height(),
        1_000_000.0 / decoder.frame_duration_us() as f64,
    );

    while let Some(frame) = decoder.next_frame()? {
        if let WbmPixels::Rgba(pixels) = &frame.video.pixels {
            // upload `pixels` to a texture, etc.
            let _ = pixels;
        }
        for chunk in &frame.audio {
            // chunk.samples — interleaved s16 PCM at chunk.sample_rate
            let _ = chunk;
        }
    }
    Ok(())
}
```
