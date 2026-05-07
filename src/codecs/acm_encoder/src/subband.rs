//! Forward subband / lifting filter for ACM encoding.
//!
//! Faithful Rust port of DLTCEP's `subband.cpp` / `subband.h` — the
//! `C_DOpt_SubbandCoder` ("delta-optimal") variant, which is the one
//! `snd2acm.cpp` selects by default with `tp = 'O'`. Original is GPL by
//! Abel Cheung / TeamX, included with TeoTwawki/dltcep.
//!
//! The transform builds a pyramid of `levels` resolutions, with subband
//! count `(1 << levels) - 1` (e.g. `levels = 7` → 127 subbands). The
//! decoder's `juggle_block` is the inverse of the same pyramid; together
//! they let the packer emit a compact representation that the existing
//! `infinitier_acm_decoder` reads back.
//!
//! Only the forward transform lives here. The packer (block quantizer +
//! per-column filler-book selection) is a separate stage that consumes
//! the `i64` coefficients this filter produces.

/// One per-subband circular FIR queue. Each queue holds `f_len` partial
/// accumulators (`f64` cells) into which `add_value` smears one input
/// sample's contribution across all taps; after `f_len` calls each cell
/// has been touched by `f_len` distinct inputs and is the "ready" output.
#[derive(Clone, Copy, Default, Debug)]
struct FilterQueue {
    /// Index in [`SubbandCoder::incomp`] of this queue's first cell.
    incomp_base: usize,
    /// Position 0..f_len of the current head within the circular buffer.
    cur_no: usize,
    /// Number of values fed to this queue so far (used to decide when
    /// the priming / latency phase ends and outputs become valid).
    input_count: u64,
    /// `input_count` parity — chooses lp vs hp coefficients for
    /// alternating samples (the polyphase split).
    is_odd: bool,
}

/// One pyramid level. Level `i` owns `1 << i` subbands and round-robins
/// across them as input arrives.
#[derive(Clone, Copy, Default, Debug)]
struct FilterLevel {
    /// Index of this level's first queue inside [`SubbandCoder::queues`].
    first: usize,
    /// `1 << level_index`.
    count: usize,
    /// Round-robin position within the level, 0..count.
    cur_no: usize,
}

/// Forward FIR pyramidal subband coder (delta-optimal variant).
pub(crate) struct SubbandCoder {
    f_half: usize,
    f_len: usize,
    levels: usize,
    subbands: usize,
    divisor: f64,
    lp_filter: Vec<f64>,
    hp_filter: Vec<f64>,
    incomp: Vec<f64>,
    queues: Vec<FilterQueue>,
    level_states: Vec<FilterLevel>,
}

impl SubbandCoder {
    /// Build the filter coefficients for `f_half`-tap (per side) D-Opt
    /// filters and `levels` pyramid levels, then prime the per-subband
    /// queues.
    pub(crate) fn new(f_half: usize, levels: usize) -> Self {
        assert!(f_half >= 1, "f_half must be ≥ 1");
        let f_len = (f_half << 1) - 1;
        // The original code's `(1 << levels) - 1`. For `levels = 0` we
        // skip the transform entirely (filter_data is a passthrough).
        let subbands = if levels == 0 { 0 } else { (1usize << levels) - 1 };

        let mut sc = Self {
            f_half,
            f_len,
            levels,
            subbands,
            divisor: 1.0,
            lp_filter: vec![0.0; f_len],
            hp_filter: vec![0.0; f_len],
            incomp: vec![0.0; f_len * subbands],
            queues: vec![FilterQueue::default(); subbands],
            level_states: vec![FilterLevel::default(); levels],
        };
        sc.build_dopt_taps();
        sc.complete_filter();
        sc.reset_queues();
        sc
    }

    /// Number of trailing zero samples the encoder must feed past the
    /// real audio so every input frame has had time to propagate
    /// through every pyramid level — `f_half * subbands` (matches
    /// `CSubbandCoder::get_init_size` in C++).
    pub(crate) fn init_size(&self) -> usize {
        self.f_half * self.subbands
    }

    #[cfg(test)]
    pub(crate) fn subbands(&self) -> usize {
        self.subbands
    }

    #[cfg(test)]
    pub(crate) fn lp_filter(&self) -> &[f64] {
        &self.lp_filter
    }

    #[cfg(test)]
    pub(crate) fn hp_filter(&self) -> &[f64] {
        &self.hp_filter
    }

    /// Forward-transform `data` in place into `res`. On return, only
    /// the first `n` entries of `res` (where `n` is the returned count)
    /// are valid — the tail holds stale values from an earlier pyramid
    /// pass and should be ignored.
    ///
    /// At least `init_size()` samples of latency are eaten across all
    /// levels: the caller is expected to feed `samples + init_size()`
    /// values total (padding the tail with zeros) so the returned count
    /// equals the original audio sample count.
    pub(crate) fn filter_data(&mut self, data: &[i16], res: &mut Vec<i64>) -> usize {
        res.clear();
        res.extend(data.iter().map(|&s| s as i64));
        if self.levels == 0 {
            return res.len();
        }
        let mut count = res.len();
        for l in 0..self.levels {
            let mut output_idx = 0usize;
            // Iterate over the current `count` items at the head of res.
            for input_idx in 0..count {
                let val = res[input_idx];

                // Pick the queue this input feeds (round-robin within
                // the level for l > 0; level 0 has a single queue).
                let q_idx = {
                    let lv = &self.level_states[l];
                    lv.first + lv.cur_no
                };

                let next_val = self.add_value(val, q_idx);

                // Test the *pre-increment* input_count, matching the C++
                // — output is valid once we've absorbed `f_half` values.
                if self.queues[q_idx].input_count >= self.f_half as u64 {
                    res[output_idx] = next_val;
                    output_idx += 1;
                }

                // Advance the queue: zero the just-consumed head cell
                // (so future add_value calls accumulate into a clean
                // slot) and bump cur_no with circular wrap. The `+1`
                // bumps to the *next* head, which is now the freshly
                // zeroed cell.
                let pos = self.queues[q_idx].incomp_base + self.queues[q_idx].cur_no;
                self.incomp[pos] = 0.0;
                let cur_no = self.queues[q_idx].cur_no + 1;
                self.queues[q_idx].cur_no = if cur_no == self.f_len { 0 } else { cur_no };
                self.queues[q_idx].input_count += 1;
                self.queues[q_idx].is_odd = !self.queues[q_idx].is_odd;

                // Round-robin within the level; level 0 has just one queue.
                if l != 0 {
                    let lv = &mut self.level_states[l];
                    lv.cur_no += 1;
                    if lv.cur_no == lv.count {
                        lv.cur_no = 0;
                    }
                }
            }
            count = output_idx;
        }
        count
    }

    /// Smear `val` across all `f_len` taps of the chosen polyphase
    /// branch (lp for even-parity inputs, hp for odd) into the queue's
    /// circular buffer. Returns the value at the queue's current head,
    /// which is the next "ready" output coefficient.
    fn add_value(&mut self, val: i64, q_idx: usize) -> i64 {
        let f_len = self.f_len;
        let (base, cur_no, is_odd) = {
            let q = &self.queues[q_idx];
            (q.incomp_base, q.cur_no, q.is_odd)
        };
        if val != 0 {
            let valf = val as f64;
            for i in 0..f_len {
                let pos = base + (cur_no + i) % f_len;
                let coeff = if is_odd {
                    self.hp_filter[i]
                } else {
                    self.lp_filter[i]
                };
                self.incomp[pos] += coeff * valf;
            }
        }
        // (long) cast in C++ truncates toward zero; Rust's `as i64`
        // matches that for finite f64 within range. The packer clamps
        // anything that would overflow i16 anyway.
        self.incomp[base + cur_no] as i64
    }

    /// `clear_filter` — re-zero the per-subband buffers and re-link the
    /// queues to their slots in `incomp`. Called once at construction;
    /// callers that want to reuse the same `SubbandCoder` for multiple
    /// streams should call it again between streams.
    fn reset_queues(&mut self) {
        if self.levels == 0 {
            return;
        }
        for v in self.incomp.iter_mut() {
            *v = 0.0;
        }
        for (i, q) in self.queues.iter_mut().enumerate() {
            *q = FilterQueue {
                incomp_base: self.f_len * i,
                cur_no: 0,
                input_count: 0,
                is_odd: false,
            };
        }
        let mut first = 0usize;
        for (i, lv) in self.level_states.iter_mut().enumerate() {
            let count = 1usize << i;
            *lv = FilterLevel {
                first,
                count,
                cur_no: 0,
            };
            first += count;
        }
    }

    /// `C_DOpt_SubbandCoder::allocate_coeffs` — Pascal-triangle-like
    /// recurrence that fills the lower half of `lp_filter` plus the
    /// centre tap. `complete_filter` mirrors the upper half and builds
    /// `hp_filter`.
    ///
    /// `old_val` is intentionally hoisted out of the per-`s` loop body
    /// to mirror the C++ — that code declares `double old_val;` inside
    /// the loop without initializing it, but in practice the same stack
    /// slot is reused across iterations so the value at the start of
    /// iteration `s` equals `lp_filter[s-1]` from iteration `s-1`. The
    /// C++ filter outputs assume that behaviour. We make the lifetime
    /// explicit so we don't depend on UB.
    fn build_dopt_taps(&mut self) {
        let f_half = self.f_half;
        let lp = &mut self.lp_filter;
        for v in lp.iter_mut() {
            *v = 0.0;
        }
        if lp.is_empty() {
            return;
        }
        lp[0] = 1.0;

        let mut old_val: f64 = 0.0;
        for s in 1..f_half {
            lp[s - 1] = 2.0 * lp[s - 1] + if s > 1 { lp[s - 2] } else { 0.0 };
            if (s - 1) % 2 == 0 {
                lp[s - 1] += 1.0;
            }

            let mut i = s as i64 - 2;
            while i >= 0 {
                let iu = i as usize;
                if iu.is_multiple_of(2) {
                    lp[iu] = old_val;
                } else {
                    old_val = lp[iu];
                    lp[iu] = 2.0 * lp[iu] + lp[iu - 1];
                }
                i -= 1;
            }

            lp[s] = 2.0 * lp[s - 1] + if s > 1 { lp[s - 2] } else { 0.0 };
            lp[s] += 1.0;
            old_val = lp[s];
        }

        // divisor = lp[0] * lp[1] / 2 — must be set *before*
        // complete_filter normalizes by it. f_half == 1 has no lp[1]
        // and the transform is degenerate; guard against the OOB.
        self.divisor = if f_half >= 2 {
            lp[0] * lp[1] / 2.0
        } else {
            1.0
        };
    }

    /// `complete_filter` — sign-flip every "i ≡ f_half-3 (mod 4)" pair,
    /// mirror the lower half into the upper half, normalize by
    /// `divisor`, then derive `hp_filter` by sign-flipping every other
    /// tap of `lp_filter` (parity matched to `f_half`).
    fn complete_filter(&mut self) {
        let f_half = self.f_half as i64;
        let f_len = self.f_len;
        let divisor = self.divisor;

        // Sign-flip pairs of taps starting from f_half-3 going down by 4.
        let mut i = f_half - 3;
        while i >= 0 {
            self.lp_filter[i as usize] = -self.lp_filter[i as usize];
            if i > 0 {
                self.lp_filter[i as usize - 1] = -self.lp_filter[i as usize - 1];
            }
            i -= 4;
        }

        // Mirror the lower half into the upper half (the centre tap at
        // f_half - 1 is its own mirror image and is left alone).
        for i in 0..self.f_half - 1 {
            self.lp_filter[f_len - i - 1] = self.lp_filter[i];
        }

        // Normalize and build hp.
        let f_half_parity = self.f_half % 2;
        for i in 0..f_len {
            self.lp_filter[i] /= divisor;
            self.hp_filter[i] = self.lp_filter[i];
            if i % 2 == f_half_parity {
                self.hp_filter[i] = -self.hp_filter[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn levels_zero_is_passthrough() {
        let mut sc = SubbandCoder::new(11, 0);
        assert_eq!(sc.subbands(), 0);
        assert_eq!(sc.init_size(), 0);

        let input: Vec<i16> = vec![100, -200, 300, -400, 500];
        let mut out = Vec::<i64>::new();
        let n = sc.filter_data(&input, &mut out);
        assert_eq!(n, input.len());
        for (i, &v) in input.iter().enumerate() {
            assert_eq!(out[i], v as i64);
        }
    }

    #[test]
    fn lp_filter_is_symmetric() {
        // After complete_filter, lp_filter[i] should equal
        // lp_filter[f_len - i - 1] for i in 0..f_half - 1.
        let sc = SubbandCoder::new(11, 7);
        let lp = sc.lp_filter();
        let f_len = lp.len();
        let f_half = sc.f_half;
        for i in 0..f_half - 1 {
            assert!(
                approx(lp[i], lp[f_len - i - 1], 1e-12),
                "lp[{i}] = {} should mirror lp[{}] = {}",
                lp[i],
                f_len - i - 1,
                lp[f_len - i - 1]
            );
        }
    }

    #[test]
    fn hp_filter_negates_alternate_taps_of_lp() {
        let sc = SubbandCoder::new(11, 7);
        let f_half_parity = sc.f_half % 2;
        for i in 0..sc.lp_filter().len() {
            let lp = sc.lp_filter()[i];
            let hp = sc.hp_filter()[i];
            if i % 2 == f_half_parity {
                assert!(approx(hp, -lp, 1e-12), "hp[{i}] should be -lp[{i}]");
            } else {
                assert!(approx(hp, lp, 1e-12), "hp[{i}] should equal lp[{i}]");
            }
        }
    }

    #[test]
    fn zero_input_produces_zero_output() {
        let mut sc = SubbandCoder::new(11, 7);
        let input = vec![0i16; sc.init_size() + 256];
        let mut out = Vec::<i64>::new();
        let n = sc.filter_data(&input, &mut out);
        assert!(n > 0, "output count must be positive for non-trivial input");
        for v in &out[..n] {
            assert_eq!(*v, 0, "zero input must produce zero output");
        }
    }

    #[test]
    fn filter_data_count_consumes_init_size() {
        // Per snd2acm.cpp's flow, feeding `samples + init_size()`
        // through filter_data should return roughly `samples`
        // coefficients — the priming / latency consumes init_size.
        let sc_levels = 7;
        let f_half = 11;
        let mut sc = SubbandCoder::new(f_half, sc_levels);

        let real_samples = 4096;
        let total = real_samples + sc.init_size();
        let input: Vec<i16> = (0..total)
            .map(|i| ((i as f64 * 0.05).sin() * 16000.0) as i16)
            .collect();

        let mut out = Vec::<i64>::new();
        let n = sc.filter_data(&input, &mut out);

        // The exact count depends on per-level rounding, but the latency
        // shaved off should equal init_size to within a small margin.
        let lost = total - n;
        let init = sc.init_size();
        assert!(
            lost.abs_diff(init) <= sc_levels,
            "expected init_size ≈ {init} samples of latency, got {lost}",
        );
    }

    #[test]
    fn dc_input_concentrates_into_first_subband() {
        // A constant (DC) input should propagate almost entirely
        // through the low-pass branch: the first chunk of outputs
        // (coarsest subband) carries the energy, the rest stay
        // small — a basic sanity check that lp/hp are wired correctly.
        let mut sc = SubbandCoder::new(11, 4);
        let total = 4096 + sc.init_size();
        let input = vec![10_000i16; total];
        let mut out = Vec::<i64>::new();
        let n = sc.filter_data(&input, &mut out);
        assert!(n > 0);

        // With levels=4 → 15 subbands, the first ~1/16 of outputs are
        // the coarsest (low-pass) subband.
        let coarse = n / 16;
        let coarse_max: i64 = out[..coarse].iter().map(|v| v.abs()).max().unwrap_or(0);
        let detail_max: i64 = out[coarse..n]
            .iter()
            .map(|v| v.abs())
            .max()
            .unwrap_or(0);

        // The coarsest subband should hold the bulk of the DC energy.
        assert!(
            coarse_max > detail_max,
            "DC should concentrate into the low-pass subband: coarse_max={coarse_max}, detail_max={detail_max}",
        );
    }
}
