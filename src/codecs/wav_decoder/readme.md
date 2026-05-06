# infinitier_wav_decoder

Streaming decoder for the two `*.WAV` flavours shipped by Infinity Engine games:

- **Real WAV** — standard `RIFF`/`WAVE` PCM, decoded via [`hound`].
- **WAVC** — a 28-byte Interplay header (`'WAVC'`, `'V1.0'`, sizes,
  channel/rate metadata, pointer to the ACM payload) wrapped around an
  Interplay ACM stream. WAVC files are 22050 Hz / 16-bit and ship in BIFs
  and the override folder under the `.WAV` extension. Despite the
  extension, they are not RIFF.

`WavDecoder::open` autodetects the flavour from the first four bytes of a
[`DataSource`] and delegates the actual decoding either to a `hound`
`WavReader` (RIFF) or to [`infinitier_acm_decoder`] (WAVC).

The decoder mirrors `AcmDecoder`'s streaming API: bytes are pulled from
the source on demand, so memory use stays bounded regardless of file
size.

## Usage

### Stream samples block-by-block

```rust,no_run
use infinitier_wav_decoder::WavDecoder;
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dec = WavDecoder::open(&DataSource::new("sound.wav"))?;
    let info = dec.info();
    println!("{} ch, {} Hz, {} samples", info.channels, info.sample_rate, info.total_values);

    // Pull a block at a time straight into a mixer buffer.
    let mut buf = [0i16; 4096];
    loop {
        let n = dec.read_samples(&mut buf)?;
        if n == 0 { break; }
        // ... feed buf[..n] to the audio backend ...
    }
    Ok(())
}
```

### Decode all samples / write to a real WAV file

```rust,no_run
use infinitier_wav_decoder::WavDecoder;
use infinitier_datasource::DataSource;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dec = WavDecoder::open(&DataSource::new("sound.wav"))?;
    let pcm: Vec<i16> = dec.decode_all()?;          // interleaved s16 PCM

    dec.reset()?;                                   // rewind to the start
    dec.decode_to_file("normalised.wav")?;          // streamed RIFF write
    Ok(())
}
```

## Limitations

- The RIFF path currently accepts only 16-bit signed-integer PCM (the
  format used by Infinity Engine sounds). Other bit depths and float
  samples are rejected with `WavError::UnsupportedPcmFormat`.
