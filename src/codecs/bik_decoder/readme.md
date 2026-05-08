# infinitier_bik_decoder

Pure-Rust decoder for **Bink Video v1** (`BIKi`), the format the IWD2 cutscenes
ship under (despite their `.mve` extension).

The implementation is a port of FFmpeg's `libavcodec/bink.c` and
`binkaudio.c` (release/6.1 snapshot).

Supported variants in scope:

| Codec | What it covers |
|---|---|
| `binkvideo` (BIKi/BIKf/BIKg/BIKh/BIKk) | Bink Video v1 with YUV420p output, optional alpha |
| `binkvideo` (BIKb) | BinkB |
| `binkaudio_dct` | DCT-based Bink audio |
| `binkaudio_rdft` | RDFT-based Bink audio |

`binkvideo2` (KB2) is out of scope.

## Usage

Unlike the streaming `MveDecoder`, this crate exposes the demuxer
(`parse_header`) and the codec (`VideoDecoder`, `AudioDecoder`) separately
so the caller controls how packets are read. The pattern is: parse the
header to get a frame index, seek to each frame's offset, then split the
packet into its `audio_packet_len`-prefixed audio block followed by the
video bitstream.

### Decode video and audio frame by frame

```rust,no_run
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use infinitier_bik_decoder::{AudioDecoder, VideoDecoder, parse_header};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut f = File::open("intro.bik")?;
    let header = parse_header(&mut f)?;

    println!(
        "{}x{} @ {:.2} fps",
        header.width,
        header.height,
        header.fps(),
    );

    let mut video = VideoDecoder::new(&header)?;
    let mut audio = header
        .audio_tracks
        .first()
        .map(AudioDecoder::new)
        .transpose()?;
    let has_audio = audio.is_some();

    let mut packet = Vec::with_capacity(header.max_frame_size as usize);
    for fr in &header.frames {
        packet.resize(fr.size as usize, 0);
        f.seek(SeekFrom::Start(fr.pos as u64))?;
        f.read_exact(&mut packet)?;

        // When audio is present, the packet starts with a u32 audio length
        // followed by that many bytes of audio bitstream; the video bytes
        // run from there to the end. Files without audio skip the prefix.
        let video_bytes = if has_audio {
            let aud_len = u32::from_le_bytes(
                [packet[0], packet[1], packet[2], packet[3]]
            ) as usize;
            let pcm: Vec<i16> = audio.as_mut().unwrap().decode_packet(&packet[4..4 + aud_len])?;
            // pcm: interleaved L,R,L,R,... (or mono).
            let _ = pcm;
            &packet[4 + aud_len..]
        } else {
            &packet[..]
        };

        let frame = video.decode_frame(video_bytes)?;
        // `frame.y / frame.u / frame.v` are YUV420p `Plane`s; each plane
        // exposes `data: Vec<u8>`, a `stride` (≥ width), and the logical
        // `width / height`. Convert to RGB downstream as you like.
        let _y_plane: &[u8] = &frame.y.data;
    }
    Ok(())
}
```

### Extract the audio track to a WAV file

```rust,no_run
use infinitier_bik_decoder::extract_audio_to_wav;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    extract_audio_to_wav("intro.bik", "intro_audio.wav")?;
    Ok(())
}
```

If the input has no audio track the destination is still created — as an
empty stereo / 22050 Hz PCM-WAV — so the path always exists after the
call returns.
