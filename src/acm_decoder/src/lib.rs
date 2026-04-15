//! ACM audio decoder — Interplay ACM format.
//!
//! Ported from the C implementation by Marko Kreen (libacm).

use std::io::Read;

const ACM_ID: u32 = 0x032897;
const WAVC_ID: u32 = 0x564157; // 'WAV' little-endian

/// Index into ampbuf where midbuf[0] lives.  Negative midbuf indices map to
/// ampbuf positions below this offset; positive ones map above it.
const MIDBUF_OFFSET: usize = 0x8000;

// Lookup tables for the variable-length Huffman-like fillers.
const MAP_1BIT: [i32; 2] = [-1, 1];
const MAP_2BIT_NEAR: [i32; 4] = [-2, -1, 1, 2];
const MAP_2BIT_FAR: [i32; 4] = [-3, -2, 2, 3];
const MAP_3BIT: [i32; 8] = [-4, -3, -2, -1, 1, 2, 3, 4];

// ─── Error type ──────────────────────────────────────────────────────────────

pub type Result<T> = std::result::Result<T, AcmError>;

#[derive(Debug)]
pub enum AcmError {
    NotAcm,
    Corrupt,
    UnexpectedEof,
    Io(std::io::Error),
}

impl std::fmt::Display for AcmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcmError::NotAcm => write!(f, "not an ACM file"),
            AcmError::Corrupt => write!(f, "corrupt ACM data"),
            AcmError::UnexpectedEof => write!(f, "unexpected end of file"),
            AcmError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for AcmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let AcmError::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl From<std::io::Error> for AcmError {
    fn from(e: std::io::Error) -> Self {
        AcmError::Io(e)
    }
}

// ─── Public info struct ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AcmInfo {
    pub channels: u32,
    pub rate: u32,
    pub acm_id: u32,
    pub acm_version: u32,
    /// Channel count as stored in the header (may differ from `channels` when
    /// force_chans is used).
    pub acm_channels: u32,
    pub acm_level: u32,
    pub acm_cols: u32,
    pub acm_rows: u32,
}

// ─── Decoder ─────────────────────────────────────────────────────────────────

pub struct AcmDecoder {
    pub info: AcmInfo,
    /// Total number of PCM words (samples × channels) in the stream.
    pub total_values: u32,

    // ── bitstream state ──
    data: Vec<u8>,
    data_pos: usize,
    bit_data: u32,
    bit_avail: u32,

    // ── block buffers ──
    block: Vec<i32>,
    wrapbuf: Vec<i32>,
    /// 0x10000-element amplitude lookup table.  `midbuf[i]` is `ampbuf[MIDBUF_OFFSET + i]`.
    ampbuf: Vec<i32>,
    block_len: usize,

    block_ready: bool,
    wavc_file: bool,
    stream_pos: u32,
    block_pos: usize,
}

impl AcmDecoder {
    /// Open an ACM stream from any `Read` source.
    ///
    /// `force_chans`:
    /// - `> 0` — force that many channels regardless of header
    /// - `0`   — trust the header channel count
    /// - `-1`  — quirk mode: force stereo for plain ACM, trust header for WAVC
    pub fn open<R: Read>(mut reader: R, force_chans: i32) -> Result<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        // The C implementation appends one zero byte at EOF so that the bit
        // reader can complete any in-progress partial word without triggering a
        // false UnexpectedEof.  We add 4 bytes for the same reason.
        data.extend([0u8; 4]);

        let mut dec = AcmDecoder {
            info: AcmInfo {
                channels: 0,
                rate: 0,
                acm_id: 0,
                acm_version: 0,
                acm_channels: 0,
                acm_level: 0,
                acm_cols: 0,
                acm_rows: 0,
            },
            total_values: 0,
            data,
            data_pos: 0,
            bit_data: 0,
            bit_avail: 0,
            block: Vec::new(),
            wrapbuf: Vec::new(),
            ampbuf: vec![0i32; 0x10000],
            block_len: 0,
            block_ready: false,
            wavc_file: false,
            stream_pos: 0,
            block_pos: 0,
        };

        dec.read_header()?;

        // Apply channel override.
        if force_chans > 0 {
            dec.info.channels = force_chans as u32;
        } else if force_chans == -1 && !dec.wavc_file && dec.info.channels < 2 {
            dec.info.channels = 2;
        }

        // Compute derived dimensions.
        dec.info.acm_cols = 1 << dec.info.acm_level;
        let wrapbuf_len = (2 * dec.info.acm_cols as usize).saturating_sub(2);
        dec.block_len = dec.info.acm_rows as usize * dec.info.acm_cols as usize;

        dec.block = vec![0i32; dec.block_len];
        // Allocate at least 1 element so the Vec is never empty.
        dec.wrapbuf = vec![0i32; wrapbuf_len.max(1)];

        Ok(dec)
    }

    // ── bitstream helpers ────────────────────────────────────────────────────

    /// Load the next up-to-4 bytes from the data array as a little-endian word.
    /// Returns `(word, bits_loaded)`.
    fn load_next_word(&mut self) -> (u32, u32) {
        let n = (self.data.len() - self.data_pos).min(4);
        let mut word = 0u32;
        for i in 0..n {
            word |= (self.data[self.data_pos + i] as u32) << (i * 8);
        }
        self.data_pos += n;
        (word, (n * 8) as u32)
    }

    /// Extract `bits` bits from the bitstream (bits must be ≤ 31).
    pub(crate) fn get_bits(&mut self, bits: u32) -> Result<u32> {
        if self.bit_avail >= bits {
            let mask = (1u32 << bits) - 1;
            let val = self.bit_data & mask;
            self.bit_data >>= bits;
            self.bit_avail -= bits;
            return Ok(val);
        }
        self.get_bits_slow(bits)
    }

    /// Slow path: reload the bit register and extract.
    fn get_bits_slow(&mut self, bits: u32) -> Result<u32> {
        let saved = self.bit_data; // bits already available
        let got = self.bit_avail;
        let remaining = bits - got;

        let (b_data, b_avail) = self.load_next_word();

        if b_avail < remaining {
            return Err(AcmError::UnexpectedEof);
        }

        let mask = (1u32 << remaining) - 1;
        let val = saved | ((b_data & mask) << got);
        self.bit_data = b_data >> remaining;
        self.bit_avail = b_avail - remaining;
        Ok(val)
    }

    // ── header parsing ───────────────────────────────────────────────────────

    fn read_wavc_header(&mut self) -> Result<()> {
        // 12 × u16 follow 'WAVC'
        let mut buf = [0u16; 12];
        for b in &mut buf {
            *b = self.get_bits(16)? as u16;
        }
        // First four bytes must be 'V1.0'  (0x3156, 0x302E)
        if buf[0] != 0x3156 || buf[1] != 0x302E {
            return Err(AcmError::NotAcm);
        }
        // buf[6] must be 28 (header length magic)
        if buf[6] != 28 {
            return Err(AcmError::NotAcm);
        }
        self.wavc_file = true;
        Ok(())
    }

    fn read_header(&mut self) -> Result<()> {
        let mut tmp = self.get_bits(24)?;

        if tmp == WAVC_ID {
            // WAVC variant: 'WAVC' = 0x43564157
            if self.get_bits(8)? != b'C' as u32 {
                return Err(AcmError::NotAcm);
            }
            self.read_wavc_header()?;
            tmp = self.get_bits(24)?;
        }

        if tmp != ACM_ID {
            return Err(AcmError::NotAcm);
        }
        self.info.acm_id = tmp;

        self.info.acm_version = self.get_bits(8)?;
        if self.info.acm_version != 1 {
            return Err(AcmError::NotAcm);
        }

        // total_values stored as two 16-bit halves (lo, hi)
        let lo = self.get_bits(16)?;
        let hi = self.get_bits(16)?;
        self.total_values = lo | (hi << 16);
        if self.total_values == 0 {
            return Err(AcmError::NotAcm);
        }

        self.info.channels = self.get_bits(16)?;
        if !(1..=2).contains(&self.info.channels) {
            return Err(AcmError::NotAcm);
        }
        self.info.acm_channels = self.info.channels;

        self.info.rate = self.get_bits(16)?;
        if self.info.rate < 4096 {
            return Err(AcmError::NotAcm);
        }

        self.info.acm_level = self.get_bits(4)?;
        self.info.acm_rows = self.get_bits(12)?;
        if self.info.acm_rows == 0 {
            return Err(AcmError::NotAcm);
        }

        Ok(())
    }

    // ── amplitude table helper ───────────────────────────────────────────────

    /// Write `block[row * acm_cols + col] = ampbuf[MIDBUF_OFFSET + idx]`.
    fn set_pos(&mut self, row: usize, col: usize, idx: i32) {
        let block_idx = (row << self.info.acm_level as usize) + col;
        let amp_idx = (MIDBUF_OFFSET as i32 + idx) as usize;
        self.block[block_idx] = self.ampbuf[amp_idx];
    }

    // ── block decoding ───────────────────────────────────────────────────────

    /// Decode one compressed block.  Returns `Ok(true)` on success,
    /// `Ok(false)` on normal end-of-stream, `Err(…)` on a real error.
    fn decode_block(&mut self) -> Result<bool> {
        self.block_ready = false;
        self.block_pos = 0;

        // Read block header — a natural EOF here is normal.
        let pwr = match self.get_bits(4) {
            Ok(v) => v,
            Err(AcmError::UnexpectedEof) => return Ok(false),
            Err(e) => return Err(e),
        };
        let val = match self.get_bits(16) {
            Ok(v) => v as i32,
            Err(AcmError::UnexpectedEof) => return Ok(false),
            Err(e) => return Err(e),
        };

        // Build the amplitude lookup table (midbuf = ampbuf[MIDBUF_OFFSET…]).
        // midbuf[i]  = i * val   for i in 0..count
        // midbuf[-i] = -i * val  for i in 1..=count
        let count = 1usize << pwr;
        let mut x = 0i32;
        for i in 0..count {
            self.ampbuf[MIDBUF_OFFSET + i] = x;
            x = x.wrapping_add(val);
        }
        x = val.wrapping_neg();
        for i in 1..=count {
            self.ampbuf[MIDBUF_OFFSET - i] = x;
            x = x.wrapping_sub(val);
        }

        // Decode columns.
        match self.fill_block() {
            Ok(()) => {}
            Err(AcmError::UnexpectedEof) => return Ok(false),
            Err(e) => return Err(e),
        }

        // Apply the lifting-scheme inverse transform.
        juggle_block(
            &mut self.wrapbuf,
            &mut self.block,
            self.info.acm_level,
            self.info.acm_rows,
            self.info.acm_cols,
        );

        self.block_ready = true;
        Ok(true)
    }

    fn fill_block(&mut self) -> Result<()> {
        let cols = self.info.acm_cols as usize;
        for col in 0..cols {
            let ind = self.get_bits(5)? as usize;
            self.fill_column(ind, col)?;
        }
        Ok(())
    }

    /// Dispatch to the appropriate filler for filler index `ind`.
    fn fill_column(&mut self, ind: usize, col: usize) -> Result<()> {
        let rows = self.info.acm_rows as usize;

        match ind {
            // ── f_zero ──────────────────────────────────────────────────────
            0 => {
                for row in 0..rows {
                    self.set_pos(row, col, 0);
                }
            }

            // ── f_bad ───────────────────────────────────────────────────────
            1 | 2 | 25 | 28 | 30 | 31 => return Err(AcmError::Corrupt),

            // ── f_linear (ind = 3..=16) ─────────────────────────────────────
            3..=16 => {
                let middle = 1i32 << (ind - 1);
                for row in 0..rows {
                    let b = self.get_bits(ind as u32)? as i32;
                    self.set_pos(row, col, b - middle);
                }
            }

            // ── f_k13 ───────────────────────────────────────────────────────
            // Huffman: 0→(zero,zero)  10→zero  11?→±1
            17 => {
                let mut i = 0;
                while i < rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        if i < rows {
                            self.set_pos(i, col, 0);
                            i += 1;
                        }
                        continue;
                    }
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        continue;
                    }
                    let b = self.get_bits(1)? as usize;
                    self.set_pos(i, col, MAP_1BIT[b]);
                    i += 1;
                }
            }

            // ── f_k12 ───────────────────────────────────────────────────────
            // Huffman: 0→zero  1?→±1
            18 => {
                for row in 0..rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(row, col, 0);
                    } else {
                        let b = self.get_bits(1)? as usize;
                        self.set_pos(row, col, MAP_1BIT[b]);
                    }
                }
            }

            // ── f_t15 ───────────────────────────────────────────────────────
            // 5 bits encode 3 ternary values: v = x1 + x2*3 + x3*9
            19 => {
                let mut i = 0;
                while i < rows {
                    let b = self.get_bits(5)?;
                    if b >= 27 {
                        return Err(AcmError::Corrupt);
                    }
                    let n1 = (b % 3) as i32 - 1;
                    let tmp = b / 3;
                    let n2 = (tmp % 3) as i32 - 1;
                    let n3 = (tmp / 3) as i32 - 1;
                    self.set_pos(i, col, n1);
                    i += 1;
                    if i >= rows {
                        break;
                    }
                    self.set_pos(i, col, n2);
                    i += 1;
                    if i >= rows {
                        break;
                    }
                    self.set_pos(i, col, n3);
                    i += 1;
                }
            }

            // ── f_k24 ───────────────────────────────────────────────────────
            // Huffman: 0→(zero,zero)  10→zero  11??→±1,±2
            20 => {
                let mut i = 0;
                while i < rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        if i < rows {
                            self.set_pos(i, col, 0);
                            i += 1;
                        }
                        continue;
                    }
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        continue;
                    }
                    let b = self.get_bits(2)? as usize;
                    self.set_pos(i, col, MAP_2BIT_NEAR[b]);
                    i += 1;
                }
            }

            // ── f_k23 ───────────────────────────────────────────────────────
            // Huffman: 0→zero  1??→±1,±2
            21 => {
                for row in 0..rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(row, col, 0);
                    } else {
                        let b = self.get_bits(2)? as usize;
                        self.set_pos(row, col, MAP_2BIT_NEAR[b]);
                    }
                }
            }

            // ── f_t27 ───────────────────────────────────────────────────────
            // 7 bits encode 3 quinary values: v = x1 + x2*5 + x3*25
            22 => {
                let mut i = 0;
                while i < rows {
                    let b = self.get_bits(7)?;
                    if b >= 125 {
                        return Err(AcmError::Corrupt);
                    }
                    let n1 = (b % 5) as i32 - 2;
                    let tmp = b / 5;
                    let n2 = (tmp % 5) as i32 - 2;
                    let n3 = (tmp / 5) as i32 - 2;
                    self.set_pos(i, col, n1);
                    i += 1;
                    if i >= rows {
                        break;
                    }
                    self.set_pos(i, col, n2);
                    i += 1;
                    if i >= rows {
                        break;
                    }
                    self.set_pos(i, col, n3);
                    i += 1;
                }
            }

            // ── f_k35 ───────────────────────────────────────────────────────
            // Huffman: 0→(zero,zero)  10→zero  110?→±1  111??→±2,±3
            23 => {
                let mut i = 0;
                while i < rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        if i < rows {
                            self.set_pos(i, col, 0);
                            i += 1;
                        }
                        continue;
                    }
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        continue;
                    }
                    if self.get_bits(1)? == 0 {
                        let b = self.get_bits(1)? as usize;
                        self.set_pos(i, col, MAP_1BIT[b]);
                    } else {
                        let b = self.get_bits(2)? as usize;
                        self.set_pos(i, col, MAP_2BIT_FAR[b]);
                    }
                    i += 1;
                }
            }

            // ── f_k34 ───────────────────────────────────────────────────────
            // Huffman: 0→zero  10?→±1  11??→±2,±3
            24 => {
                for row in 0..rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(row, col, 0);
                        continue;
                    }
                    if self.get_bits(1)? == 0 {
                        let b = self.get_bits(1)? as usize;
                        self.set_pos(row, col, MAP_1BIT[b]);
                    } else {
                        let b = self.get_bits(2)? as usize;
                        self.set_pos(row, col, MAP_2BIT_FAR[b]);
                    }
                }
            }

            // ── f_k45 ───────────────────────────────────────────────────────
            // Huffman: 0→(zero,zero)  10→zero  11???→±1..±4
            26 => {
                let mut i = 0;
                while i < rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        if i < rows {
                            self.set_pos(i, col, 0);
                            i += 1;
                        }
                        continue;
                    }
                    if self.get_bits(1)? == 0 {
                        self.set_pos(i, col, 0);
                        i += 1;
                        continue;
                    }
                    let b = self.get_bits(3)? as usize;
                    self.set_pos(i, col, MAP_3BIT[b]);
                    i += 1;
                }
            }

            // ── f_k44 ───────────────────────────────────────────────────────
            // Huffman: 0→zero  1???→±1..±4
            27 => {
                for row in 0..rows {
                    if self.get_bits(1)? == 0 {
                        self.set_pos(row, col, 0);
                    } else {
                        let b = self.get_bits(3)? as usize;
                        self.set_pos(row, col, MAP_3BIT[b]);
                    }
                }
            }

            // ── f_t37 ───────────────────────────────────────────────────────
            // 7 bits encode 2 values in base-11: v = x1 + x2*11
            29 => {
                let mut i = 0;
                while i < rows {
                    let b = self.get_bits(7)?;
                    if b >= 121 {
                        return Err(AcmError::Corrupt);
                    }
                    let n1 = (b % 11) as i32 - 5;
                    let n2 = (b / 11) as i32 - 5;
                    self.set_pos(i, col, n1);
                    i += 1;
                    if i >= rows {
                        break;
                    }
                    self.set_pos(i, col, n2);
                    i += 1;
                }
            }

            _ => return Err(AcmError::Corrupt),
        }

        Ok(())
    }

    // ── public decode API ────────────────────────────────────────────────────

    /// Decode the entire ACM stream and return all PCM samples as signed 16-bit
    /// values in interleaved channel order (same as s16le WAV data).
    pub fn decode_all(&mut self) -> Result<Vec<i16>> {
        let mut samples: Vec<i16> = Vec::with_capacity(self.total_values as usize);
        let shift = self.info.acm_level;
        let channels = self.info.channels as usize;

        while self.stream_pos < self.total_values {
            if !self.block_ready {
                if !self.decode_block()? {
                    break; // natural EOF
                }
            }

            let avail = self.block_len - self.block_pos;
            let remaining = (self.total_values - self.stream_pos) as usize;
            let mut n = avail.min(remaining);

            // Keep stereo pairs aligned.
            if channels > 1 {
                n -= n % channels;
            }

            if n == 0 {
                // Can't make progress (channel alignment edge case); skip block.
                self.block_ready = false;
                continue;
            }

            for i in 0..n {
                let v = self.block[self.block_pos + i] >> shift;
                samples.push(v.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
            }

            self.stream_pos += n as u32;
            self.block_pos += n;
            if self.block_pos == self.block_len {
                self.block_ready = false;
            }
        }

        // Pad with zeros to reach the declared total_values count, mirroring
        // the C tool's behaviour (it writes the expected byte count in the WAV
        // header and zero-fills any gap at the end).
        samples.resize(self.total_values as usize, 0i16);

        Ok(samples)
    }
}

// ─── Juggle (inverse lifting transform) ──────────────────────────────────────

/// One pass of the inverse lifting step.
///
/// Mirrors the C `juggle()` function exactly:
/// - For each of the `sub_len` columns:
///   - Load the two wrap registers from `wrapbuf[wrap_start + col*2 ..]`
///   - Iterate over `sub_count/2` row pairs in `block`, updating them in-place
///   - Save the registers back to wrapbuf
///
/// All arithmetic wraps around (u32) to match the C unsigned-int behaviour.
fn juggle(
    wrapbuf: &mut [i32],
    block: &mut [i32],
    wrap_start: usize,
    block_start: usize,
    sub_len: usize,
    sub_count: usize,
) {
    for col in 0..sub_len {
        let wi = wrap_start + col * 2;
        let mut r0 = wrapbuf[wi] as u32;
        let mut r1 = wrapbuf[wi + 1] as u32;

        let mut p = block_start + col;
        for _ in 0..sub_count / 2 {
            let r2 = block[p] as u32;
            block[p] = r1.wrapping_mul(2).wrapping_add(r0.wrapping_add(r2)) as i32;
            p += sub_len;

            let r3 = block[p] as u32;
            block[p] = r2.wrapping_mul(2).wrapping_sub(r1.wrapping_add(r3)) as i32;
            p += sub_len;

            r0 = r2;
            r1 = r3;
        }

        wrapbuf[wi] = r0 as i32;
        wrapbuf[wi + 1] = r1 as i32;
    }
}

/// Apply the complete hierarchical juggle transform to the decoded block.
///
/// Mirrors `juggle_block()` from the C source.  The wrapbuf is stateful across
/// calls (it carries "edge" samples between blocks), so it must not be cleared
/// between blocks.
fn juggle_block(
    wrapbuf: &mut [i32],
    block: &mut [i32],
    acm_level: u32,
    acm_rows: u32,
    acm_cols: u32,
) {
    if acm_level == 0 {
        return;
    }

    let step_subcount: usize = if acm_level > 9 {
        1
    } else {
        ((2048u32 >> acm_level) - 2) as usize
    };

    let mut todo_count = acm_rows as usize;
    let mut block_offset = 0usize;

    loop {
        let sub_count_base = step_subcount.min(todo_count);
        let mut sub_len = acm_cols as usize / 2;
        let mut sub_count = sub_count_base * 2;

        let mut wrap_offset = 0usize;

        // First juggle pass at the coarsest scale.
        juggle(
            wrapbuf,
            block,
            wrap_offset,
            block_offset,
            sub_len,
            sub_count,
        );
        wrap_offset += sub_len * 2;

        // Increment the first element of each row in this chunk.
        // (Matches: `for (i=0,p=block_p; i<sub_count; i++) { p[0]++; p+=sub_len; }`)
        for i in 0..sub_count {
            block[block_offset + i * sub_len] += 1;
        }

        // Finer-scale passes, halving sub_len each time.
        while sub_len > 1 {
            sub_len /= 2;
            sub_count *= 2;
            juggle(
                wrapbuf,
                block,
                wrap_offset,
                block_offset,
                sub_len,
                sub_count,
            );
            wrap_offset += sub_len * 2;
        }

        if todo_count <= step_subcount {
            break;
        }
        todo_count -= step_subcount;
        block_offset += step_subcount << acm_level as usize;
    }
}
