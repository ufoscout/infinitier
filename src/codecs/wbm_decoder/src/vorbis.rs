//! Vorbis driver: Xiph-laced `CodecPrivate` parsing + symphonia's
//! `VorbisDecoder`.
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
//! `VorbisDecoder::try_new` wants `extra_data = ident || setup` (no
//! length prefixes, no comment header), so we sniff channels/sample-rate
//! out of the ident header for our public surface, drop the comment
//! header on the floor, and feed the concatenation in. Each Matroska
//! audio block is then handed verbatim to `Decoder::decode` as a fresh
//! `Packet` with zero ts/dur — symphonia's Vorbis decoder doesn't use
//! either field.

use symphonia::core::codecs::audio::well_known::CODEC_ID_VORBIS;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::packet::PacketRef;
use symphonia::core::units::{Duration, Timestamp};
use symphonia::default::codecs::VorbisDecoder;

use crate::error::{WbmError, WbmResult};

pub(crate) struct VorbisDriver {
    decoder: VorbisDecoder,
    pub channels: u8,
    pub sample_rate: u32,
}

impl VorbisDriver {
    pub fn from_codec_private(buf: &[u8]) -> WbmResult<Self> {
        let (ident, _comment, setup) = parse_xiph_lacing(buf)?;
        let (channels, sample_rate) = sniff_ident(&ident)?;

        let mut extra_data = Vec::with_capacity(ident.len() + setup.len());
        extra_data.extend_from_slice(&ident);
        extra_data.extend_from_slice(&setup);

        let mut params = AudioCodecParameters::new();
        params
            .for_codec(CODEC_ID_VORBIS)
            .with_extra_data(extra_data.into_boxed_slice());

        let decoder = VorbisDecoder::try_new(&params, &AudioDecoderOptions::default())
            .map_err(WbmError::Vorbis)?;

        Ok(Self {
            decoder,
            channels,
            sample_rate,
        })
    }

    /// Decode one Matroska audio block. Returns one `Vec<i16>` per
    /// channel.
    pub fn decode(&mut self, packet: &[u8]) -> WbmResult<Vec<Vec<i16>>> {
        let pkt = PacketRef::new(0, Timestamp::new(0), Duration::new(0), packet);
        let buf_ref = self.decoder.decode_ref(&pkt).map_err(WbmError::Vorbis)?;
        let mut planes: Vec<Vec<i16>> = Vec::new();
        buf_ref.copy_to_vecs_planar::<i16>(&mut planes);
        Ok(planes)
    }
}

/// Sniff `audio_channels` and `audio_sample_rate` straight out of the
/// Vorbis identification header. Layout (Vorbis I §4.2.2):
///
/// ```text
/// 0      packet_type = 0x01
/// 1..=6  "vorbis"
/// 7..11  vorbis_version (u32 LE)  — must be 0
/// 11     audio_channels (u8)
/// 12..16 audio_sample_rate (u32 LE)
/// ```
fn sniff_ident(buf: &[u8]) -> WbmResult<(u8, u32)> {
    if buf.len() < 16 || buf[0] != 0x01 || &buf[1..7] != b"vorbis" {
        return Err(WbmError::BadXiphLacing(
            "ident header missing 0x01 'vorbis' preamble".into(),
        ));
    }
    let channels = buf[11];
    let sample_rate = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    Ok((channels, sample_rate))
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

    #[test]
    fn sniff_ident_extracts_channels_and_rate() {
        // 0x01 || "vorbis" || version=0 || channels=2 || rate=48000
        let mut buf = vec![0x01];
        buf.extend_from_slice(b"vorbis");
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.push(2u8);
        buf.extend_from_slice(&48_000u32.to_le_bytes());
        let (ch, sr) = sniff_ident(&buf).unwrap();
        assert_eq!(ch, 2);
        assert_eq!(sr, 48_000);
    }
}
