//! Vorbis driver: Xiph-laced `CodecPrivate` parsing + lewton's inline
//! `audio::read_audio_packet` API.
//!
//! Matroska doesn't store the three Vorbis init headers (`identification`,
//! `comment`, `setup`) inside Ogg pages — it bakes them into a single
//! `CodecPrivate` blob using the Xiph lacing scheme:
//!
//! ```text
//! byte 0:        N = number of EXTRA headers (= 2 for Vorbis)
//! bytes 1..p:    N length-prefix encoded sizes — each as zero or more
//!                0xff bytes followed by one < 0xff byte; total = sum.
//! bytes p..:     header 1 || header 2 || header 3 (length implicit)
//! ```
//!
//! After extraction the three headers go through lewton's parser, then
//! every Matroska audio packet is fed verbatim into
//! [`lewton::audio::read_audio_packet`] which returns the decoded
//! per-channel samples.

use lewton::{
    audio::{PreviousWindowRight, read_audio_packet},
    header::{IdentHeader, SetupHeader, read_header_comment, read_header_ident, read_header_setup},
};

use crate::error::{WbmError, WbmResult};

pub(crate) struct VorbisDriver {
    ident: IdentHeader,
    setup: SetupHeader,
    pwr: PreviousWindowRight,
    pub channels: u8,
    pub sample_rate: u32,
}

impl VorbisDriver {
    pub fn from_codec_private(buf: &[u8]) -> WbmResult<Self> {
        let (h1, h2, h3) = parse_xiph_lacing(buf)?;
        let ident = read_header_ident(&h1)?;
        let _comment = read_header_comment(&h2)?;
        let setup = read_header_setup(
            &h3,
            ident.audio_channels,
            (ident.blocksize_0, ident.blocksize_1),
        )?;
        let channels = ident.audio_channels;
        let sample_rate = ident.audio_sample_rate;
        Ok(Self {
            ident,
            setup,
            pwr: PreviousWindowRight::new(),
            channels,
            sample_rate,
        })
    }

    /// Decode one Matroska audio block. Returns one `Vec<i16>` per
    /// channel (lewton's native shape).
    pub fn decode(&mut self, packet: &[u8]) -> WbmResult<Vec<Vec<i16>>> {
        let pcm = read_audio_packet(&self.ident, &self.setup, packet, &mut self.pwr)?;
        Ok(pcm)
    }
}

/// Split the Xiph-laced `CodecPrivate` blob into its three constituent
/// Vorbis headers (`identification`, `comment`, `setup`).
fn parse_xiph_lacing(buf: &[u8]) -> WbmResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    if buf.is_empty() {
        return Err(WbmError::BadXiphLacing("empty CodecPrivate".into()));
    }
    let n = buf[0];
    if n != 2 {
        return Err(WbmError::BadXiphLacing(format!(
            "unexpected header count {n} (Vorbis requires 2 extra-headers)"
        )));
    }

    let mut p = 1usize;
    let len1 = read_lacing(buf, &mut p)?;
    let len2 = read_lacing(buf, &mut p)?;

    if p + len1 + len2 > buf.len() {
        return Err(WbmError::BadXiphLacing(
            "header lengths exceed CodecPrivate buffer".into(),
        ));
    }
    let h1 = buf[p..p + len1].to_vec();
    let h2 = buf[p + len1..p + len1 + len2].to_vec();
    let h3 = buf[p + len1 + len2..].to_vec();
    Ok((h1, h2, h3))
}

fn read_lacing(buf: &[u8], cursor: &mut usize) -> WbmResult<usize> {
    let mut total = 0usize;
    loop {
        if *cursor >= buf.len() {
            return Err(WbmError::BadXiphLacing(
                "lacing length runs past buffer end".into(),
            ));
        }
        let b = buf[*cursor];
        *cursor += 1;
        total += b as usize;
        if b < 0xff {
            return Ok(total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xiph_lacing_short_lengths() {
        // 2 extra headers, len1 = 30, len2 = 25, then 30+25+10 bytes.
        let mut buf = vec![2u8, 30, 25];
        buf.extend(std::iter::repeat_n(0xAAu8, 30));
        buf.extend(std::iter::repeat_n(0xBBu8, 25));
        buf.extend(std::iter::repeat_n(0xCCu8, 10));
        let (h1, h2, h3) = parse_xiph_lacing(&buf).unwrap();
        assert_eq!(h1.len(), 30);
        assert_eq!(h2.len(), 25);
        assert_eq!(h3.len(), 10);
        assert!(h1.iter().all(|&b| b == 0xAA));
        assert!(h2.iter().all(|&b| b == 0xBB));
        assert!(h3.iter().all(|&b| b == 0xCC));
    }

    #[test]
    fn xiph_lacing_long_length() {
        // len1 = 0xff + 0xff + 0x05 = 515; one byte len2 = 1; 1 byte h3.
        let mut buf = vec![2u8, 0xff, 0xff, 0x05, 1];
        buf.extend(std::iter::repeat_n(0u8, 515));
        buf.push(7u8);
        buf.push(8u8);
        let (h1, h2, h3) = parse_xiph_lacing(&buf).unwrap();
        assert_eq!(h1.len(), 515);
        assert_eq!(h2, vec![7u8]);
        assert_eq!(h3, vec![8u8]);
    }

    #[test]
    fn xiph_lacing_rejects_bad_count() {
        let buf = vec![1u8, 0, 0];
        assert!(parse_xiph_lacing(&buf).is_err());
    }
}
