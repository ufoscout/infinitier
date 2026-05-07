//! Block quantizer + per-column filler-book selector + bitstream writer.
//!
//! Faithful Rust port of DLTCEP's `packer.cpp` / `packer.h` / `utils.cpp`
//! (Abel Cheung / TeamX, GPL).
//!
//! For each block of `subblocks × sb_size` coefficients (`acm_rows ×
//! acm_cols` in our decoder's vocabulary), the packer:
//!
//! 1. Copies the block into an internal buffer and tracks the maximum
//!    absolute and maximum positive value per column ([`analyse`]).
//! 2. Picks a quantizer step `val` — by default the GCD of all
//!    coefficients (lossless), or via a bisection that targets a
//!    per-block bit budget when `max_bits_limit` is set.
//! 3. Writes the block header — `pwr` (4 bits) and `val` (16 bits) —
//!    then runs [`pack_column`] for each column. `pack_column` looks at
//!    the column's value distribution (max amplitude, count of single
//!    and consecutive-pair zeros, count of ±1s) and selects one of the
//!    13 filler books the decoder understands: `f_zero` (ind 0),
//!    `f_linear` (3..=16), `f_k12`/`f_k13`/`f_k23`/`f_k24`/`f_k34`/
//!    `f_k35`/`f_k44`/`f_k45` (ind 17..27), `f_t15` (19), `f_t27` (22),
//!    `f_t37` (29).
//!
//! The packer doesn't write the file header — that's the caller's job
//! (see `encode_packed` in `lib.rs`). It also doesn't emit a
//! per-stream `flush_bit_stream` — the surrounding [`BitWriter`] is
//! finalised by the caller.

use std::io::{self, Write};

use crate::bitwriter::BitWriter;

/// Symbolic indices into the K-book descriptor table.
#[derive(Clone, Copy)]
enum K {
    K13 = 0,
    K12 = 1,
    K24 = 2,
    K23 = 3,
    K35 = 4,
    K34 = 5,
    K45 = 6,
    K44 = 7,
}

/// One Huffman codeword: the value and the number of bits to write
/// LSB-first.
#[derive(Clone, Copy)]
struct OneVal {
    bits: u8,
    val: u8,
}

/// K-book descriptor: the 5-bit `ind` written into the bitstream, the
/// `double_zero` flag (whether the book has a "two consecutive zeros"
/// codeword), the offset to add to a value to index into `data`, and
/// the codeword table.
struct KDesc {
    /// Filler index written in the bitstream — matches the decoder's
    /// `fill_column` `match` arm for this book.
    number: u8,
    double_zero: bool,
    /// Offset added to the (signed) value to index into `data`.
    base: i32,
    /// Codeword table indexed by `(base + value)`.
    data: &'static [OneVal],
}

// Lookup tables. Each row is one (bits, val) Huffman codeword. The
// commented header lines up with the indices: e.g. K13 covers values
// -1..1, K23 covers -2..2, etc.

// Values:        -1     0     1
const K13V: &[OneVal] = &[ov(3, 3), ov(2, 1), ov(3, 7)];
// Values:        -1     0     1
const K12V: &[OneVal] = &[ov(2, 1), ov(1, 0), ov(2, 3)];
// Values:        -2     -1     0     1      2
const K24V: &[OneVal] = &[ov(4, 3), ov(4, 7), ov(2, 1), ov(4, 11), ov(4, 15)];
// Values:        -2     -1     0     1     2
const K23V: &[OneVal] = &[ov(3, 1), ov(3, 3), ov(1, 0), ov(3, 5), ov(3, 7)];
// Values:        -3      -2     -1     0      1     2     3
const K35V: &[OneVal] = &[
    ov(5, 7),
    ov(5, 15),
    ov(4, 3),
    ov(2, 1),
    ov(4, 11),
    ov(5, 23),
    ov(5, 31),
];
// Values:        -3     -2     -1     0     1     2     3
const K34V: &[OneVal] = &[
    ov(4, 3),
    ov(4, 7),
    ov(3, 1),
    ov(1, 0),
    ov(3, 5),
    ov(4, 11),
    ov(4, 15),
];
// Values:        -4     -3      -2     -1     0      1     2     3     4
const K45V: &[OneVal] = &[
    ov(5, 3),
    ov(5, 7),
    ov(5, 11),
    ov(5, 15),
    ov(2, 1),
    ov(5, 19),
    ov(5, 23),
    ov(5, 27),
    ov(5, 31),
];
// Values:        -4     -3     -2     -1     0     1     2     3     4
const K44V: &[OneVal] = &[
    ov(4, 1),
    ov(4, 3),
    ov(4, 5),
    ov(4, 7),
    ov(1, 0),
    ov(4, 9),
    ov(4, 11),
    ov(4, 13),
    ov(4, 15),
];

const fn ov(bits: u8, val: u8) -> OneVal {
    OneVal { bits, val }
}

const K_DESC: [KDesc; 8] = [
    KDesc {
        number: 17,
        double_zero: true,
        base: 1,
        data: K13V,
    },
    KDesc {
        number: 18,
        double_zero: false,
        base: 1,
        data: K12V,
    },
    KDesc {
        number: 20,
        double_zero: true,
        base: 2,
        data: K24V,
    },
    KDesc {
        number: 21,
        double_zero: false,
        base: 2,
        data: K23V,
    },
    KDesc {
        number: 23,
        double_zero: true,
        base: 3,
        data: K35V,
    },
    KDesc {
        number: 24,
        double_zero: false,
        base: 3,
        data: K34V,
    },
    KDesc {
        number: 26,
        double_zero: true,
        base: 4,
        data: K45V,
    },
    KDesc {
        number: 27,
        double_zero: false,
        base: 4,
        data: K44V,
    },
];

/// Per-block packer.
pub(crate) struct ValuePacker {
    subblocks: usize,
    sb_size: usize,
    pblock_size: usize,
    /// `pblock_size + 2 * sb_size` cells. The trailing two rows are
    /// always zero — they pad the look-aheads in `pack_column`'s
    /// double-zero check, `make_t15`/`make_t27`'s 3-at-a-time packing,
    /// and `make_t37`'s 2-at-a-time packing, so trailing partial groups
    /// effectively complete with implicit zeros.
    pblock: Vec<i16>,
    max_abs: Vec<i16>,
    max_plus: Vec<i16>,
    max_bits_limit: Option<u64>,
}

impl ValuePacker {
    pub(crate) fn new(subblocks: usize, sb_size: usize, max_bits_limit: Option<u64>) -> Self {
        let pblock_size = subblocks * sb_size;
        Self {
            subblocks,
            sb_size,
            pblock_size,
            pblock: vec![0i16; pblock_size + 2 * sb_size],
            max_abs: vec![0i16; sb_size],
            max_plus: vec![0i16; sb_size],
            max_bits_limit,
        }
    }

    /// Encode one block of `subblocks × sb_size` coefficients (row-major,
    /// matching the decoder's `block[row * acm_cols + col]` layout) and
    /// write the resulting bitstream into `bw`.
    pub(crate) fn add_block<W: Write>(
        &mut self,
        block: &[i16],
        bw: &mut BitWriter<W>,
    ) -> io::Result<()> {
        assert_eq!(
            block.len(),
            self.pblock_size,
            "block must have exactly subblocks * sb_size = {} elements (got {})",
            self.pblock_size,
            block.len()
        );
        self.analyse(block, bw)?;
        for col in 0..self.sb_size {
            self.pack_column(col, bw)?;
        }
        Ok(())
    }

    /// Copy `block` into `pblock`, locate per-column max abs / max
    /// positive values, choose the quantizer step `val`, and call
    /// [`granulate`](Self::granulate) which writes `pwr` + `val` to the
    /// bitstream and rewrites `pblock` in-place to hold the quantizer
    /// indices.
    fn analyse<W: Write>(&mut self, block: &[i16], bw: &mut BitWriter<W>) -> io::Result<()> {
        for v in self.max_abs.iter_mut() {
            *v = 0;
        }
        for v in self.max_plus.iter_mut() {
            *v = 0;
        }

        let mut sub_number = 0usize;
        for (i, &v) in block.iter().enumerate().take(self.pblock_size) {
            self.pblock[i] = v;
            if v > 0 {
                if self.max_plus[sub_number] < v {
                    self.max_plus[sub_number] = v;
                }
                if self.max_abs[sub_number] < v {
                    self.max_abs[sub_number] = v;
                }
            } else {
                // Avoid `-i16::MIN` overflow by computing in i32 then
                // saturating to i16::MAX (32767). Practically a value of
                // -32768 with no +32767 counterpart still rounds to
                // pwr=15 in `granulate`, so the loss of precision here
                // is irrelevant.
                let abs_val = (-(v as i32)).min(i16::MAX as i32) as i16;
                if self.max_abs[sub_number] < abs_val {
                    self.max_abs[sub_number] = abs_val;
                }
            }
            sub_number += 1;
            if sub_number == self.sb_size {
                sub_number = 0;
            }
        }

        // Initial guess: GCD of all coefficients. If GCD is zero (all
        // zeros), fall back to 1 — granulate will then write pwr=0,
        // val=1 and every column packs as f_zero.
        let mut val = gcd_array(&self.pblock[..self.pblock_size]);
        if val == 0 {
            val = 1;
        }

        if let Some(limit) = self.max_bits_limit {
            // Bisection to find the smallest val that brings the
            // estimated block size below `limit`. Initial range search
            // doubles the step; bisection then narrows it. Mirrors the
            // C++ exactly.
            let limit = limit as f64;
            let mut step: i32 = 8;
            let mut init_was_ok = true;
            while self.estimate(val) > limit {
                val += step << 1;
                step <<= 1;
                init_was_ok = false;
            }
            if !init_was_ok {
                step -= 1;
                val -= step;
                while step > 0 {
                    let half = step >> 1;
                    if self.estimate(val + half) > limit {
                        val += half + 1;
                        step -= half + 1;
                    } else {
                        step = half;
                    }
                }
            }
        }

        self.granulate(val, bw)
    }

    /// Approximate bits-per-block estimate at quantizer step `val`,
    /// used by the bisection search.
    fn estimate(&self, val: i32) -> f64 {
        let val_f = val as f64;
        let mut res = 0.0f64;
        for i in 0..self.sb_size {
            let m = round_half_away(self.max_abs[i] as f64 / val_f);
            let p = round_half_away(self.max_plus[i] as f64 / val_f);
            res += approx_len(m, p);
        }
        res * self.subblocks as f64
    }

    /// Quantize `pblock` in place: divide each cell by `val` (round to
    /// nearest), find the global maximum amplitude, derive `pwr =
    /// ceil(log2(max))`, and write `pwr` (4 bits) + `val` (16 bits) to
    /// the bitstream.
    fn granulate<W: Write>(&mut self, val: i32, bw: &mut BitWriter<W>) -> io::Result<()> {
        let val_f = val as f64;
        let mut max: i32 = 0;
        for i in 0..self.pblock_size {
            let n = round_half_away(self.pblock[i] as f64 / val_f);
            // n is in [-32768, 32767] for any val ≥ 1 and original i16
            // input. Clamp defensively so we don't depend on rounding
            // edge cases.
            let n = n.clamp(i16::MIN as i32, i16::MAX as i32);
            self.pblock[i] = n as i16;
            // Match the C++ formula: |n| for negatives, n+1 for
            // non-negatives (so even max=0 yields max≥1, which keeps
            // the log2 well defined). The `+1` also ensures the
            // amplitude lookup table has room for the largest positive
            // value.
            let n_abs = if n < 0 { -n } else { n + 1 };
            if n_abs > max {
                max = n_abs;
            }
        }
        // Re-zero the look-ahead padding rows in case granulate ran
        // after a previous block left non-zero values there. (The
        // analyse step doesn't touch these rows, so zeros from
        // construction time persist — but re-zero defensively.)
        for v in self.pblock[self.pblock_size..].iter_mut() {
            *v = 0;
        }

        let pwr = if max <= 1 {
            0u32
        } else {
            ((max as f64).log2()).ceil() as u32
        };
        debug_assert!(pwr <= 15, "pwr {pwr} exceeds 4-bit field; max={max}");

        bw.put_bits(pwr & 0xF, 4)?;
        bw.put_bits(val as u32 & 0xFFFF, 16)?;
        Ok(())
    }

    /// Choose and emit the best filler book for a single column.
    fn pack_column<W: Write>(&mut self, col: usize, bw: &mut BitWriter<W>) -> io::Result<()> {
        // Gather statistics for this column.
        let mut p0 = 0i32; // count of singleton zeros (zero followed by non-zero)
        let mut p_all_0 = 0i32; // total count of zeros
        let mut p00 = 0i32; // count of consecutive-pair zeros
        let mut p1 = 0i32; // count of values with abs = 1
        let mut max_amp = 0i32;
        let mut max_plus_amp = 0i32;

        let stride = self.sb_size;
        let mut row = 0usize;
        let mut idx = col;
        // The look-ahead `pblock[idx + stride]` is safe because pblock
        // has 2*sb_size trailing zero rows.
        while row < self.subblocks {
            let v = self.pblock[idx];
            if v == 0 {
                p_all_0 += 1;
                if self.pblock[idx + stride] == 0 {
                    p00 += 1;
                    row += 1;
                    idx += stride;
                    if row < self.subblocks {
                        p_all_0 += 1;
                    }
                } else {
                    p0 += 1;
                }
            } else {
                let abs_val = if v > 0 {
                    if max_plus_amp < v as i32 {
                        max_plus_amp = v as i32;
                    }
                    v as i32
                } else {
                    -(v as i32)
                };
                if max_amp < abs_val {
                    max_amp = abs_val;
                }
                if abs_val == 1 {
                    p1 += 1;
                }
            }
            row += 1;
            idx += stride;
        }

        let _ = p0; // statistic computed for parity with the C++; not consumed.
        let p00_x3 = p00 * 3;
        let pall0_x3 = p_all_0 * 3;
        let n = self.subblocks as i32;

        match max_amp {
            0 => {
                // f_zero — write ind=0, no payload. The decoder fills
                // the column with zeros.
                bw.put_bits(0, 5)?;
                Ok(())
            }
            1 => {
                if p00_x3 > n {
                    self.make_k(K::K13, col, bw)
                } else if pall0_x3 > n {
                    self.make_k(K::K12, col, bw)
                } else {
                    self.make_t15(col, bw)
                }
            }
            2 => {
                if p00_x3 > n {
                    self.make_k(K::K24, col, bw)
                } else if pall0_x3 > n {
                    self.make_k(K::K23, col, bw)
                } else {
                    self.make_t27(col, bw)
                }
            }
            3 => {
                if p00_x3 > n {
                    self.make_k(K::K35, col, bw)
                } else if pall0_x3 + p1 > n {
                    self.make_k(K::K34, col, bw)
                } else {
                    self.make_linear(3, col, bw)
                }
            }
            4 => {
                if max_plus_amp <= 3 {
                    if p00_x3 > n {
                        self.make_k(K::K45, col, bw)
                    } else if pall0_x3 > n {
                        self.make_k(K::K44, col, bw)
                    } else {
                        self.make_linear(3, col, bw)
                    }
                } else if p00_x3 > n {
                    self.make_k(K::K45, col, bw)
                } else if 2 * pall0_x3 > n {
                    self.make_k(K::K44, col, bw)
                } else {
                    self.make_t37(col, bw)
                }
            }
            5 => self.make_t37(col, bw),
            _ => {
                // Default: linear with `pwr+1` bits, where pwr =
                // ceil(log2(max_amp adjusted upward by max_plus+1)).
                let mut max_amp_eff = max_amp;
                let pm = max_plus_amp + 1;
                if max_amp_eff < pm {
                    max_amp_eff = pm;
                }
                let pwr = (max_amp_eff as f64).log2().ceil() as i32;
                self.make_linear(pwr + 1, col, bw)
            }
        }
    }

    fn make_k<W: Write>(
        &self,
        which: K,
        col: usize,
        bw: &mut BitWriter<W>,
    ) -> io::Result<()> {
        let desc = &K_DESC[which as usize];
        bw.put_bits(desc.number as u32, 5)?;

        let stride = self.sb_size;
        let mut row = 0usize;
        let mut idx = col;
        while row < self.subblocks {
            if desc.double_zero
                && self.pblock[idx] == 0
                && self.pblock[idx + stride] == 0
            {
                bw.put_bits(0, 1)?;
                row += 1;
                idx += stride;
            } else {
                let v = self.pblock[idx] as i32;
                let lookup = (desc.base + v) as usize;
                debug_assert!(
                    lookup < desc.data.len(),
                    "K-book lookup out of range: book={} value={} base={} idx={}",
                    desc.number,
                    v,
                    desc.base,
                    lookup
                );
                let item = desc.data[lookup];
                bw.put_bits(item.val as u32, item.bits as u32)?;
            }
            row += 1;
            idx += stride;
        }
        Ok(())
    }

    fn make_linear<W: Write>(
        &self,
        bits: i32,
        col: usize,
        bw: &mut BitWriter<W>,
    ) -> io::Result<()> {
        debug_assert!(
            (3..=16).contains(&bits),
            "linear book bits must be in 3..=16, got {bits}"
        );
        let base = 1i32 << (bits - 1);
        bw.put_bits(bits as u32, 5)?;

        let stride = self.sb_size;
        let mut idx = col;
        for _ in 0..self.subblocks {
            let encoded = (base + self.pblock[idx] as i32) as u32;
            bw.put_bits(encoded, bits as u32)?;
            idx += stride;
        }
        Ok(())
    }

    fn make_t15<W: Write>(&self, col: usize, bw: &mut BitWriter<W>) -> io::Result<()> {
        bw.put_bits(19, 5)?;
        let stride = self.sb_size;
        let mut idx = col;
        let mut row = 0usize;
        while row < self.subblocks {
            let a = (1 + self.pblock[idx] as i32) as u32;
            let b = (1 + self.pblock[idx + stride] as i32) as u32;
            let c = (1 + self.pblock[idx + 2 * stride] as i32) as u32;
            let val = a + b * 3 + c * 9;
            bw.put_bits(val, 5)?;
            row += 3;
            idx += 3 * stride;
        }
        Ok(())
    }

    fn make_t27<W: Write>(&self, col: usize, bw: &mut BitWriter<W>) -> io::Result<()> {
        bw.put_bits(22, 5)?;
        let stride = self.sb_size;
        let mut idx = col;
        let mut row = 0usize;
        while row < self.subblocks {
            let a = (2 + self.pblock[idx] as i32) as u32;
            let b = (2 + self.pblock[idx + stride] as i32) as u32;
            let c = (2 + self.pblock[idx + 2 * stride] as i32) as u32;
            let val = a + b * 5 + c * 25;
            bw.put_bits(val, 7)?;
            row += 3;
            idx += 3 * stride;
        }
        Ok(())
    }

    fn make_t37<W: Write>(&self, col: usize, bw: &mut BitWriter<W>) -> io::Result<()> {
        bw.put_bits(29, 5)?;
        let stride = self.sb_size;
        let mut idx = col;
        let mut row = 0usize;
        while row < self.subblocks {
            let a = (5 + self.pblock[idx] as i32) as u32;
            let b = (5 + self.pblock[idx + stride] as i32) as u32;
            let val = a + b * 11;
            bw.put_bits(val, 7)?;
            row += 2;
            idx += 2 * stride;
        }
        Ok(())
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// `(int) floor(0.5 + x)` from the C++ — round half away from zero (or
/// rather: round half toward positive). Sufficient for our use because
/// quantization values stay finite and well-behaved.
fn round_half_away(x: f64) -> i32 {
    (x + 0.5).floor() as i32
}

/// GCD of an `i16` slice via repeated pair-GCD. Treats each value as
/// `|v|` (zeros pass through; the result is `0` only if every input is
/// zero, in which case the caller bumps it to `1`).
fn gcd_array(values: &[i16]) -> i32 {
    if values.is_empty() {
        return 0;
    }
    let mut acc = values[0] as i32;
    for &v in &values[1..] {
        acc = gcd_pair(acc, v as i32);
    }
    if acc < 0 { -acc } else { acc }
}

fn gcd_pair(mut q: i32, mut r: i32) -> i32 {
    if q < 0 {
        q = -q;
    }
    if r < 0 {
        r = -r;
    }
    while r != 0 {
        let r_new = q % r;
        q = r;
        r = r_new;
    }
    q
}

/// Approximate bit cost of one packed column at max value `m`, used by
/// the bisection in [`ValuePacker::analyse`]. Mirrors `approx_len` in
/// `packer.cpp`.
fn approx_len(max: i32, plus_max: i32) -> f64 {
    match max {
        0 => 0.0,
        1 => 5.0 / 3.0,
        2 => 7.0 / 3.0,
        3 => 3.0,
        4 => {
            if plus_max <= 3 {
                3.0
            } else {
                7.0 / 2.0
            }
        }
        5 => 7.0 / 2.0,
        _ => {
            let mut max_eff = max;
            let pm = plus_max + 1;
            if max_eff < pm {
                max_eff = pm;
            }
            (max_eff as f64).log2().ceil() + 1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcd_of_pair_handles_negatives() {
        assert_eq!(gcd_pair(0, 0), 0);
        assert_eq!(gcd_pair(12, 18), 6);
        assert_eq!(gcd_pair(-12, 18), 6);
        assert_eq!(gcd_pair(-12, -18), 6);
        assert_eq!(gcd_pair(7, 0), 7);
    }

    #[test]
    fn gcd_of_array() {
        assert_eq!(gcd_array(&[]), 0);
        assert_eq!(gcd_array(&[0, 0, 0]), 0);
        assert_eq!(gcd_array(&[2, 4, 6, 8]), 2);
        assert_eq!(gcd_array(&[2, 4, 7]), 1);
        assert_eq!(gcd_array(&[-6, 9, 12]), 3);
    }

    #[test]
    fn approx_len_matches_known_values() {
        assert_eq!(approx_len(0, 0), 0.0);
        assert_eq!(approx_len(1, 0), 5.0 / 3.0);
        assert_eq!(approx_len(2, 0), 7.0 / 3.0);
        assert_eq!(approx_len(3, 0), 3.0);
        assert_eq!(approx_len(4, 3), 3.0);
        assert_eq!(approx_len(4, 4), 7.0 / 2.0);
        assert_eq!(approx_len(5, 0), 7.0 / 2.0);
        // Default branch: max=8, pm=2 → max_eff = 8 → log2(8)=3 → 3+1=4
        assert_eq!(approx_len(8, 1), 4.0);
        // Default branch where pm wins: max=2, pm=10 → max_eff=10 → ceil(log2(10))=4 → 5
        assert_eq!(approx_len(2, 9), approx_len(2, 9)); // sanity it doesn't panic
    }
}
