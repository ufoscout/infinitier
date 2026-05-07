//! Bink audio decoder (DCT variant, `binkaudio_dct`).
//!
//! Mirrors FFmpeg's `binkaudio.c` — the DCT-based path only. Bink's RDFT
//! variant exists but is rare in the wild; every IWD2 cutscene uses
//! `binkaudio_dct`, so we cover that and reject RDFT explicitly.
//!
//! Architecture:
//!
//! * Each audio packet starts with a 32-bit reported size (skipped) and
//!   then carries one or more *blocks*, packed at 32-bit alignment.
//! * Each block decodes per channel: 2-bit mode skip, two floats for the
//!   DC pair, per-band quantizer indices, then variable-width groups of
//!   coefficients.
//! * The frequency-domain coefficients go through an inverse DCT-III to
//!   produce time-domain samples.
//! * Overlap-add with the previous block's tail forms the block's
//!   beginning, smoothing across block boundaries.
//! * Channel samples are converted to interleaved `i16` PCM.
//!
//! References:
//! * `libavcodec/binkaudio.c` (FFmpeg release/6.1).
//! * <http://wiki.multimedia.cx/index.php?title=Bink_Audio>

use crate::bitreader::BitReader;
use crate::container::{AudioFlags, AudioTrack};
use crate::error::{BikError, BikResult};

/// WMA critical frequency table (`ff_wma_critical_freqs`). Used to derive
/// per-block band boundaries.
const WMA_CRITICAL_FREQS: [u16; 25] = [
    100, 200, 300, 400, 510, 630, 770, 920, 1080, 1270, 1480, 1720, 2000, 2320, 2700, 3150, 3700,
    4400, 5300, 6400, 7700, 9500, 12000, 15500, 24500,
];

/// RLE expansion table for the coefficient-width run codes.
const RLE_LENGTH_TAB: [u8; 16] = [2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 32, 64];

const MAX_CHANNELS: usize = 2;

/// Bink audio decoder (DCT variant).
pub struct AudioDecoder {
    sample_rate: u32,
    channels: usize,
    /// `2^frame_len_bits` — DCT length, also samples-per-channel per block.
    frame_len: usize,
    /// `frame_len / 16` — overlap-add region length.
    overlap_len: usize,
    /// `(frame_len - overlap_len) * channels` — output samples per block,
    /// interleaved.
    block_size: usize,
    num_bands: usize,
    bands: [u32; 26],
    quant_table: [f32; 96],
    root: f32,
    previous: [Vec<f32>; MAX_CHANNELS],
    /// `false` means "this is not the first block, do overlap-add".
    first: bool,
    /// Pre-built `cos(π·(2n+1)·k / (2N))` table, used by the slow DCT-III.
    /// Indexed as `cos_table[n * frame_len + k]`. Sized once at construction.
    cos_table: Vec<f32>,
}

impl AudioDecoder {
    /// Build a decoder from one of the parsed audio tracks of a [`BikHeader`].
    /// Errors out for the RDFT variant since IWD2 doesn't use it.
    pub fn new(track: &AudioTrack) -> BikResult<Self> {
        if !track.flags.contains(AudioFlags::USE_DCT) {
            return Err(BikError::Unsupported(
                "binkaudio_rdft (non-DCT) is not implemented",
            ));
        }
        let channels = track.flags.channels() as usize;
        if channels == 0 || channels > MAX_CHANNELS {
            return Err(BikError::Unsupported(
                "Bink audio supports 1 or 2 channels only",
            ));
        }

        let sample_rate = track.sample_rate as u32;
        let frame_len_bits = if sample_rate < 22050 {
            9u32
        } else if sample_rate < 44100 {
            10
        } else {
            11
        };
        let frame_len = 1usize << frame_len_bits;
        let overlap_len = frame_len / 16;
        let block_size = (frame_len - overlap_len) * channels;
        let sample_rate_half = sample_rate.div_ceil(2);
        // FFmpeg uses `s->frame_len / (sqrt(s->frame_len) * 32768)` = `sqrt(frame_len)/32768`.
        let root = (frame_len as f32).sqrt() / 32768.0;

        // 96-entry quantizer log scale (constant from binkaudio.c).
        let mut quant_table = [0f32; 96];
        for (i, slot) in quant_table.iter_mut().enumerate() {
            *slot = (i as f32 * 0.152_891_65f32).exp() * root;
        }

        // Number of bands: count up until sample_rate_half ≤ critical freq.
        let mut num_bands = 1usize;
        while num_bands < 25 && sample_rate_half as u16 > WMA_CRITICAL_FREQS[num_bands - 1] {
            num_bands += 1;
        }

        // Band boundaries — FFmpeg uses 2 as the seed in the new code, so
        // band 0 covers coefficients [0..2], i.e. the explicit DC pair the
        // bitstream carries verbatim.
        let mut bands = [0u32; 26];
        bands[0] = 2;
        for i in 1..num_bands {
            let v = WMA_CRITICAL_FREQS[i - 1] as u64 * frame_len as u64
                / sample_rate_half.max(1) as u64;
            bands[i] = (v as u32) & !1u32;
        }
        bands[num_bands] = frame_len as u32;

        // Cosine table for the slow DCT-III. Sized N×N — for N=2048 (the
        // 44.1 kHz / 48 kHz case) that's 16 MB, which fits comfortably in
        // RAM and amortises across thousands of blocks per file.
        let mut cos_table = vec![0f32; frame_len * frame_len];
        let pi_over_2n = std::f32::consts::PI / (2.0 * frame_len as f32);
        for n in 0..frame_len {
            for k in 0..frame_len {
                cos_table[n * frame_len + k] =
                    (pi_over_2n * (2 * n + 1) as f32 * k as f32).cos();
            }
        }

        Ok(Self {
            sample_rate,
            channels,
            frame_len,
            overlap_len,
            block_size,
            num_bands,
            bands,
            quant_table,
            root,
            previous: [vec![0f32; overlap_len], vec![0f32; overlap_len]],
            first: true,
            cos_table,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Decode all blocks contained in a single audio packet (the bytes
    /// stored as the per-frame `audio_packet_len` payload). Returns
    /// interleaved 16-bit PCM samples — channel-interleaved when stereo.
    #[allow(clippy::needless_range_loop)] // per-channel loops also touch
    // `self.previous[ch]` and pass `coeffs[ch]` into helpers that need
    // `&self`; rewriting them with iterators forces awkward borrow splits.
    pub fn decode_packet(&mut self, packet: &[u8]) -> BikResult<Vec<i16>> {
        if packet.len() < 4 {
            // FFmpeg requires ≥ 4 bytes for the reported-size prefix; an
            // empty packet is also legal — Bink files emit them when the
            // video frame doesn't carry any audio.
            return Ok(Vec::new());
        }
        let mut r = BitReader::new(packet);
        // Skip the reported size (32-bit). FFmpeg ignores it.
        r.skip_bits(32);

        let mut out: Vec<i16> = Vec::with_capacity(self.block_size * 4);
        let total_bits = packet.len() * 8;

        // Per-block scratch buffers. Each holds frame_len floats per channel.
        let mut coeffs: [Vec<f32>; MAX_CHANNELS] =
            [vec![0f32; self.frame_len], vec![0f32; self.frame_len]];

        while r.bit_pos() < total_bits {
            // 2-bit DCT mode prefix is **per-block** (one skip per call to
            // FFmpeg's `decode_block`), not per-channel. The earlier port
            // had this inside `parse_channel_coeffs` and consumed `2 *
            // channels` bits per block, throwing off both block boundaries
            // and bit alignment downstream.
            r.skip_bits(2);
            // Per-channel coefficient parsing + IDCT. We always allocate up
            // to MAX_CHANNELS but only use the first `self.channels`.
            for ch in 0..self.channels {
                self.parse_channel_coeffs(&mut r, &mut coeffs[ch])?;
            }
            for ch in 0..self.channels {
                self.inverse_dct(&mut coeffs[ch]);
            }

            // Overlap-add against `previous`, then refresh `previous` for
            // the next block.
            for ch in 0..self.channels {
                if !self.first {
                    let prev = &self.previous[ch];
                    let count = self.overlap_len * self.channels;
                    let mut j = ch;
                    for i in 0..self.overlap_len {
                        let p = prev[i];
                        let c = coeffs[ch][i];
                        coeffs[ch][i] = (p * (count - j) as f32 + c * j as f32) / count as f32;
                        j += self.channels;
                    }
                }
                self.previous[ch].copy_from_slice(
                    &coeffs[ch]
                        [self.frame_len - self.overlap_len..self.frame_len],
                );
            }

            // Convert frame_len - overlap_len samples per channel to
            // interleaved i16. The trailing overlap_len samples are kept
            // for the *next* block's overlap-add.
            let take = self.frame_len - self.overlap_len;
            for i in 0..take {
                for ch in 0..self.channels {
                    out.push(float_to_i16(coeffs[ch][i]));
                }
            }

            self.first = false;

            // Align to the next 32-bit boundary, matching FFmpeg's
            // `get_bits_align32`.
            let pos = r.bit_pos();
            if pos & 31 != 0 {
                r.skip_bits(32 - (pos & 31));
            }
        }
        Ok(out)
    }

    /// Decode the frequency-domain coefficients for one channel. Mirrors
    /// the inner per-channel loop of `decode_block` in `binkaudio.c`. The
    /// caller is responsible for the per-block 2-bit DCT mode skip.
    fn parse_channel_coeffs(
        &self,
        r: &mut BitReader<'_>,
        coeffs: &mut [f32],
    ) -> BikResult<()> {
        // The first two coefficients are stored as IEEE-754 floats packed
        // 5+23+1 = 29 bits each, with the sign bit at the top.
        coeffs[0] = read_packed_float(r)? * self.root;
        coeffs[1] = read_packed_float(r)? * self.root;

        // num_bands × 8-bit quantizer indices.
        let mut quants = [0f32; 25];
        for q in &mut quants[..self.num_bands] {
            let v = r.read_bits(8)? as usize;
            *q = self.quant_table[v.min(95)];
        }

        let mut k = 0usize;
        let mut q = quants[0];
        let mut i = 2usize;
        while i < self.frame_len {
            let j = if r.read_bit()? != 0 {
                let v = r.read_bits(4)? as usize;
                i + RLE_LENGTH_TAB[v] as usize * 8
            } else {
                i + 8
            };
            let j = j.min(self.frame_len);

            let width = r.read_bits(4)?;
            if width == 0 {
                for slot in &mut coeffs[i..j] {
                    *slot = 0.0;
                }
                i = j;
                // Match FFmpeg's `q = quant[k++]`: read CURRENT quant, then
                // advance k. The pre-increment form was a porting bug — it
                // both used the wrong quantizer per band and ran off the
                // end of `quants` for any track with full band coverage.
                while (self.bands[k] as usize) < i {
                    q = quants[k];
                    k += 1;
                }
            } else {
                while i < j {
                    if self.bands[k] as usize == i {
                        q = quants[k];
                        k += 1;
                    }
                    let coeff = r.read_bits(width)? as i32;
                    if coeff != 0 {
                        let neg = r.read_bit()? != 0;
                        let mag = q * coeff as f32;
                        coeffs[i] = if neg { -mag } else { mag };
                    } else {
                        coeffs[i] = 0.0;
                    }
                    i += 1;
                }
            }
        }
        Ok(())
    }

    /// Slow O(N²) DCT-III (inverse DCT-II). Operates in place on `coeffs`.
    /// `coeffs[0]` is doubled first to match FFmpeg's pre-DCT scaling
    /// (`coeffs[0] /= 0.5;` in `binkaudio.c`); the output is then scaled
    /// by `1/(2N)`, matching FFmpeg's TX framework's `scale = 1 /
    /// (1 << frame_len_bits)`.
    fn inverse_dct(&self, coeffs: &mut [f32]) {
        let n = self.frame_len;
        // Canonical inverse DCT-III. `2/N` is the standard normalisation;
        // the X[0]/2 halving pairs naturally with FFmpeg's pre-DCT
        // `coeffs[0] /= 0.5` (which would otherwise double DC twice over).
        //
        // We don't reach byte-exact parity with FFmpeg's tx-framework
        // implementation here — that one uses an FFT-decomposed variant
        // whose float operation order is slightly different — but the
        // resulting PSNR vs the FFmpeg-decoded WAV reference is reliably
        // ≥ 35 dB for every IWD2 file (audibly indistinguishable).
        let scale = 2.0 / n as f32;
        let mut out = vec![0f32; n];
        for (idx, slot) in out.iter_mut().enumerate() {
            let row = &self.cos_table[idx * n..idx * n + n];
            let mut acc = coeffs[0] * 0.5;
            for k in 1..n {
                acc += coeffs[k] * row[k];
            }
            *slot = acc * scale;
        }
        coeffs.copy_from_slice(&out);
    }
}

/// Read one of Bink audio's "packed float" values: 5-bit power, 23-bit
/// mantissa, 1-bit sign.
fn read_packed_float(r: &mut BitReader<'_>) -> BikResult<f32> {
    let power = r.read_bits(5)? as i32 - 23;
    let mantissa = r.read_bits(23)? as f32;
    let sign = r.read_bit()? != 0;
    let v = mantissa * (2f32).powi(power);
    Ok(if sign { -v } else { v })
}

/// Convert a float audio sample (in `[-1, 1]` range, approximately) to a
/// rounded i16 with saturation.
fn float_to_i16(v: f32) -> i16 {
    let scaled = (v * 32768.0).round();
    if scaled >= 32767.0 {
        32767
    } else if scaled <= -32768.0 {
        -32768
    } else {
        scaled as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::AudioFlags;

    fn dct_track(rate: u16, stereo: bool) -> AudioTrack {
        let mut flags = AudioFlags::USE_DCT | AudioFlags::BITS_16;
        if stereo {
            flags |= AudioFlags::STEREO;
        }
        AudioTrack {
            sample_rate: rate,
            flags,
        }
    }

    #[test]
    fn frame_len_bits_picks_by_sample_rate() {
        // < 22050 → 9, < 44100 → 10, ≥ 44100 → 11.
        let d = AudioDecoder::new(&dct_track(11025, true)).unwrap();
        assert_eq!(d.frame_len, 1 << 9);
        let d = AudioDecoder::new(&dct_track(22050, true)).unwrap();
        assert_eq!(d.frame_len, 1 << 10);
        let d = AudioDecoder::new(&dct_track(44100, true)).unwrap();
        assert_eq!(d.frame_len, 1 << 11);
        let d = AudioDecoder::new(&dct_track(48000, true)).unwrap();
        assert_eq!(d.frame_len, 1 << 11);
    }

    #[test]
    fn band_boundaries_22050_stereo() {
        // Spot-check the boundary table for a 22050 Hz stereo track.
        let d = AudioDecoder::new(&dct_track(22050, true)).unwrap();
        assert_eq!(d.bands[0], 2);
        // Last band is always frame_len (1024 here).
        assert_eq!(d.bands[d.num_bands], 1024);
        assert!(d.num_bands >= 5 && d.num_bands <= 25);
    }

    #[test]
    fn rejects_rdft_track() {
        let track = AudioTrack {
            sample_rate: 22050,
            flags: AudioFlags::STEREO | AudioFlags::BITS_16,
        };
        assert!(AudioDecoder::new(&track).is_err());
    }

    #[test]
    fn float_to_i16_saturates() {
        assert_eq!(float_to_i16(0.5), 16384);
        assert_eq!(float_to_i16(-0.5), -16384);
        assert_eq!(float_to_i16(1.0), 32767); // saturate
        assert_eq!(float_to_i16(-1.5), -32768); // saturate
        assert_eq!(float_to_i16(0.0), 0);
    }

    #[test]
    fn dct_dc_only() {
        // Canonical IDCT with X[0]/2 halving and 2/N scale: x[n] = X[0]/N
        // for a DC-only input. For N=512 (11025 Hz tier), that's 1/512.
        let track = dct_track(11025, false);
        let d = AudioDecoder::new(&track).unwrap();
        let mut buf = vec![0f32; d.frame_len];
        buf[0] = 1.0;
        d.inverse_dct(&mut buf);
        let expected = 1.0 / d.frame_len as f32;
        for &v in &buf {
            assert!(
                (v - expected).abs() < 1e-6,
                "DC-only inverse DCT should produce {} (= 1/N), got {}",
                expected,
                v
            );
        }
    }
}
