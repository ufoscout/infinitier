//! LSB-first bit writer that emits little-endian 32-bit words.
//!
//! Mirrors the decoder's `get_bits` view of the bitstream: every
//! [`put_bits`](BitWriter::put_bits) call appends `n` bits at the
//! current write head; once a full 32-bit word has accumulated it's
//! written out as 4 little-endian bytes. A trailing partial word is
//! flushed by [`finish`](BitWriter::finish) using only as many bytes as
//! are needed (the decoder's `read_at_most` handles short final words).

use std::io::{self, Write};

pub(crate) struct BitWriter<W: Write> {
    out: W,
    /// Pending bits, packed LSB-first. Up to 63 bits of data live here
    /// between calls — at most 31 bits before a `put_bits` plus 32 from
    /// the new value.
    buf: u64,
    bits: u32,
}

impl<W: Write> BitWriter<W> {
    pub(crate) fn new(out: W) -> Self {
        Self {
            out,
            buf: 0,
            bits: 0,
        }
    }

    pub(crate) fn put_bits(&mut self, value: u32, n: u32) -> io::Result<()> {
        debug_assert!(n <= 32);
        if n == 0 {
            return Ok(());
        }
        let mask = if n == 32 {
            !0u32
        } else {
            (1u32 << n) - 1
        };
        self.buf |= ((value & mask) as u64) << self.bits;
        self.bits += n;
        while self.bits >= 32 {
            let word = (self.buf & 0xFFFF_FFFF) as u32;
            self.out.write_all(&word.to_le_bytes())?;
            self.buf >>= 32;
            self.bits -= 32;
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> io::Result<W> {
        if self.bits > 0 {
            let word = (self.buf & 0xFFFF_FFFF) as u32;
            let n_bytes = self.bits.div_ceil(8) as usize;
            self.out.write_all(&word.to_le_bytes()[..n_bytes])?;
        }
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_simple_pattern() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            bw.put_bits(0b1010, 4).unwrap();
            bw.put_bits(0xDEAD, 16).unwrap();
            bw.put_bits(0xBEEF, 16).unwrap();
            bw.finish().unwrap();
        }
        // Expected: bits packed LSB-first into LE words.
        let mut expected = ((0xBEEFu64) << 20) | ((0xDEADu64) << 4) | 0b1010;
        let total_bits: u32 = 4 + 16 + 16;
        let n_bytes = total_bits.div_ceil(8);
        let mut got = Vec::new();
        for _ in 0..n_bytes {
            got.push((expected & 0xFF) as u8);
            expected >>= 8;
        }
        assert_eq!(buf, got);
    }
}
