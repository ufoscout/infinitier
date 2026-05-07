//! Interplay DPCM compressor — encoder side of the codec the
//! `mve_decoder` already decompresses. Each sample is encoded as a
//! 1-byte index into a fixed 256-entry signed-i16 lookup table; the
//! decoder adds the looked-up delta to a per-channel predictor and
//! saturates to `[-32768, 32767]`. The encoder must mirror that
//! arithmetic exactly to avoid predictor drift.
//!
//! The LUT below is byte-for-byte identical to
//! `mve_decoder::audio::DELTA_TABLE` — duplicated rather than
//! re-exported so the encoder doesn't take a dependency edge on the
//! decoder.

/// 256-entry signed delta lookup, matching `mve_decoder::audio`.
/// **Non-monotonic** at indices 124–128 (where the encoding flips
/// from "small positive" to "small negative"); a binary-search
/// nearest-neighbour optimisation must account for that.
pub(crate) static DELTA_TABLE: [i16; 256] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 47, 51, 56, 61, 66, 72,
    79, 86, 94, 102, 112, 122, 133, 145, 158, 173, 189, 206, 225, 245, 267, 292, 318, 348, 379,
    414, 452, 493, 538, 587, 640, 699, 763, 832, 908, 991, 1081, 1180, 1288, 1405, 1534, 1673,
    1826, 1993, 2175, 2373, 2590, 2826, 3084, 3365, 3672, 4008, 4373, 4772, 5208, 5683, 6202, 6767,
    7385, 8059, 8794, 9597, 10472, 11428, 12471, 13609, 14851, 16206, 17685, 19298, 21060, 22981,
    25078, 27367, 29864, 32589, -29973, -26728, -23186, -19322, -15105, -10503, -5481, -1, 1, 1,
    5481, 10503, 15105, 19322, 23186, 26728, 29973, -32589, -29864, -27367, -25078, -22981, -21060,
    -19298, -17685, -16206, -14851, -13609, -12471, -11428, -10472, -9597, -8794, -8059, -7385,
    -6767, -6202, -5683, -5208, -4772, -4373, -4008, -3672, -3365, -3084, -2826, -2590, -2373,
    -2175, -1993, -1826, -1673, -1534, -1405, -1288, -1180, -1081, -991, -908, -832, -763, -699,
    -640, -587, -538, -493, -452, -414, -379, -348, -318, -292, -267, -245, -225, -206, -189, -173,
    -158, -145, -133, -122, -112, -102, -94, -86, -79, -72, -66, -61, -56, -51, -47, -43, -42, -41,
    -40, -39, -38, -37, -36, -35, -34, -33, -32, -31, -30, -29, -28, -27, -26, -25, -24, -23, -22,
    -21, -20, -19, -18, -17, -16, -15, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1,
];

/// Compress an interleaved i16 PCM stream into the Interplay DPCM byte
/// format the engine expects. `channels` is 1 (mono) or 2 (stereo,
/// L,R,L,R,…). The first sample of each channel is written verbatim
/// as a 16-bit little-endian seed; every subsequent sample is encoded
/// as a 1-byte LUT index whose post-saturation predictor minimises
/// the absolute distance to the target sample.
///
/// Encoded length: `2 * min(samples.len(), channels) + (samples.len() -
/// min(samples.len(), channels))`. For typical mono input that's
/// `1 + samples.len()` bytes; for typical stereo it's `2 +
/// samples.len()` bytes.
pub(crate) fn compress(samples: &[i16], channels: u16) -> Vec<u8> {
    debug_assert!(channels == 1 || channels == 2);
    let ch_count = channels as usize;
    let n_seeds = samples.len().min(ch_count);

    let mut out = Vec::with_capacity(2 * n_seeds + samples.len().saturating_sub(n_seeds));
    let mut predictor = [0i32; 2];

    for ch in 0..n_seeds {
        let seed = samples[ch];
        out.extend_from_slice(&seed.to_le_bytes());
        predictor[ch] = seed as i32;
    }

    let mut ch = 0usize;
    for &target in samples.iter().skip(n_seeds) {
        let p = predictor[ch];
        let target_i = target as i32;
        let mut best_byte = 0u8;
        let mut best_dist = i32::MAX;
        // Brute-force search the 256-entry LUT. The non-monotonic
        // discontinuity at indices 124–128 makes binary search
        // unsafe, so a linear scan is the simple-and-correct option.
        for b in 0..256usize {
            let candidate = (p + DELTA_TABLE[b] as i32).clamp(-32768, 32767);
            let dist = (candidate - target_i).abs();
            if dist < best_dist {
                best_byte = b as u8;
                best_dist = dist;
                if dist == 0 {
                    break;
                }
            }
        }
        out.push(best_byte);
        predictor[ch] =
            (predictor[ch] + DELTA_TABLE[best_byte as usize] as i32).clamp(-32768, 32767);
        if ch_count > 1 {
            ch ^= ch_count - 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirror of the decoder's `decompress_audio` used for round-trip
    /// verification inside the encoder crate (saves the dev-dep
    /// edge). Kept tightly scoped to the unit tests below.
    fn decompress(data: &[u8], channels: u8) -> Vec<i16> {
        let mut out = Vec::new();
        let mut predictor = [0i32; 2];
        let mut pos = 0usize;
        for i in 0..channels as usize {
            if pos + 2 > data.len() {
                break;
            }
            let raw = u16::from_le_bytes([data[pos], data[pos + 1]]) as i32;
            let signed = if raw & 0x8000 != 0 { raw - 0x10000 } else { raw };
            predictor[i] = signed;
            out.push(signed as i16);
            pos += 2;
        }
        let mut ch = 0usize;
        while pos < data.len() {
            predictor[ch] = (predictor[ch] + DELTA_TABLE[data[pos] as usize] as i32)
                .clamp(-32768, 32767);
            out.push(predictor[ch] as i16);
            pos += 1;
            if channels > 1 {
                ch ^= (channels as usize) - 1;
            }
        }
        out
    }

    fn err_stats(src: &[i16], dec: &[i16]) -> (u32, i32) {
        assert_eq!(src.len(), dec.len());
        let mut sum: u64 = 0;
        let mut max: i32 = 0;
        for (&s, &d) in src.iter().zip(dec.iter()) {
            let e = (s as i32 - d as i32).abs();
            sum += e as u64;
            if e > max {
                max = e;
            }
        }
        (((sum + src.len() as u64 / 2) / src.len() as u64) as u32, max)
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert!(compress(&[], 1).is_empty());
        assert!(compress(&[], 2).is_empty());
    }

    #[test]
    fn seed_only_round_trip_mono() {
        // Single sample → just the seed, no deltas.
        let src = [12345i16];
        let bytes = compress(&src, 1);
        assert_eq!(bytes, vec![0x39, 0x30]); // 12345 = 0x3039 LE
        assert_eq!(decompress(&bytes, 1), src);
    }

    #[test]
    fn seed_only_round_trip_stereo() {
        // Stereo with exactly 2 samples — both seeds, no deltas.
        let src = [-100i16, 200];
        let bytes = compress(&src, 2);
        assert_eq!(bytes.len(), 4);
        assert_eq!(decompress(&bytes, 2), src);
    }

    #[test]
    fn silence_round_trips_exactly() {
        // All-zero input: predictor stays at 0, every delta byte 0
        // selects DELTA_TABLE[0] = 0 → exact reconstruction.
        let src = vec![0i16; 1024];
        let bytes = compress(&src, 1);
        let back = decompress(&bytes, 1);
        assert_eq!(back, src, "silence must round-trip bit-exact");
    }

    #[test]
    fn slow_ramp_round_trips_within_one_lsb() {
        // Linear ramp 0…16383 — every step is +/-1 LSB, which the
        // small-delta entries 0..43 cover exactly. Verify ≤ 1 LSB
        // mean error and ≤ 1 LSB max error.
        let src: Vec<i16> = (0..16384).map(|i| (i / 4) as i16).collect();
        let bytes = compress(&src, 1);
        let back = decompress(&bytes, 1);
        let (mean, max) = err_stats(&src, &back);
        assert!(mean <= 1, "mean abs error {mean} > 1");
        assert!(max <= 1, "max abs error {max} > 1");
    }

    #[test]
    fn sine_round_trips_within_tight_bound() {
        // 1 kHz sine at 22050 Hz, 1 second, ±4096 amplitude — close
        // to the per-sample-delta profile of the smptebars audio
        // fixture (`delta_mean ≈ 326`). The LUT's ~30-LSB
        // granularity in that range puts the best-case mean error
        // around ~15 LSB; we leave generous headroom over that.
        let mut src = Vec::with_capacity(22050);
        for n in 0..22050 {
            let t = n as f64 / 22050.0;
            let s = (4096.0 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin()) as i16;
            src.push(s);
        }
        let bytes = compress(&src, 1);
        let back = decompress(&bytes, 1);
        let (mean, max) = err_stats(&src, &back);
        assert!(mean <= 30, "mean abs error {mean} > 30");
        assert!(max <= 256, "max abs error {max} > 256");
    }

    #[test]
    fn fast_sine_stays_within_loose_bound() {
        // 4 kHz sine at 22050 Hz, ±20000 amplitude — close to the
        // worst-case per-sample slope DPCM can be asked to track.
        // Mean error grows because LUT granularity in the 5–10k
        // range is hundreds of LSB; the bound here is "doesn't
        // explode", not "high quality".
        let mut src = Vec::with_capacity(22050);
        for n in 0..22050 {
            let t = n as f64 / 22050.0;
            let s = (20000.0 * (2.0 * std::f64::consts::PI * 4000.0 * t).sin()) as i16;
            src.push(s);
        }
        let bytes = compress(&src, 1);
        let back = decompress(&bytes, 1);
        let (mean, max) = err_stats(&src, &back);
        assert!(mean <= 1500, "mean abs error {mean} > 1500");
        assert!(max <= 8000, "max abs error {max} > 8000");
    }

    #[test]
    fn stereo_channels_track_independently() {
        // L oscillates fast; R is silent. Each channel has its own
        // predictor, so R must round-trip exactly even while L moves.
        let mut src = Vec::with_capacity(2048);
        for n in 0..1024 {
            let l = (((n * 137) % 32767) - 16384) as i16;
            src.push(l);
            src.push(0); // R = silence
        }
        let bytes = compress(&src, 2);
        let back = decompress(&bytes, 2);
        assert_eq!(back.len(), src.len());
        for (i, (&s, &d)) in src.iter().zip(back.iter()).enumerate() {
            if i % 2 == 1 {
                assert_eq!(s, d, "R channel sample {i} drifted: src={s} dec={d}");
            }
        }
    }

    #[test]
    fn output_length_matches_spec() {
        // Mono with N samples → 2 (seed) + (N - 1) deltas.
        for n in 1..16 {
            let src = vec![0i16; n];
            assert_eq!(compress(&src, 1).len(), 2 + (n - 1));
        }
        // Stereo with N samples (N ≥ 2) → 4 (seeds) + (N - 2) deltas.
        for n in 2..16 {
            let src = vec![0i16; n];
            assert_eq!(compress(&src, 2).len(), 4 + (n - 2));
        }
    }
}
