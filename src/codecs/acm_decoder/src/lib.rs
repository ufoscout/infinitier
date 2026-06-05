#![doc = include_str!("../readme.md")]

use log::{debug, error};
use thiserror::Error as ThisError;

use hound::WavWriter;
use infinitier_datasource::{DataSource, DataTrait, ReadExt, Reader};

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

/// Tail-decoder selector for the eight Huffman-style fillers (modes
/// `f_k12`/`f_k13`/`f_k23`/`f_k24`/`f_k34`/`f_k35`/`f_k44`/`f_k45`).
/// Each variant identifies which trailing bits the filler reads after
/// the leading prefix, and which constant table it maps them through.
#[derive(Clone, Copy)]
enum HuffTail {
    /// 1 bit → `MAP_1BIT` (±1)
    M1,
    /// 2 bits → `MAP_2BIT_NEAR` (±1, ±2)
    M2Near,
    /// 3 bits → `MAP_3BIT` (±1..±4)
    M3,
    /// 1 prefix bit: 0 → `MAP_1BIT` (1 more bit), 1 → `MAP_2BIT_FAR`
    /// (2 more bits, ±2/±3)
    M1OrM2Far,
}

// ─── Error type ──────────────────────────────────────────────────────────────

pub type Result<T> = std::result::Result<T, AcmError>;

#[derive(Debug, ThisError)]
pub enum AcmError {
    #[error("NotAcmError")]
    NotAcmError,
    #[error("CorruptError")]
    CorruptError,
    #[error("UnexpectedEofError")]
    UnexpectedEofError,
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),
    #[error("WavCodecError: {0}")]
    WavCodecError(#[from] hound::Error),
}

impl From<AcmError> for std::io::Error {
    /// Lets callers use `?` on `AcmError`-returning helpers from inside
    /// `io::Result`-returning functions (e.g. an `Importer::import` impl).
    /// `IoError` is unwrapped to preserve the original `ErrorKind`; the rest
    /// are surfaced as `ErrorKind::Other` carrying the `AcmError` source.
    fn from(err: AcmError) -> Self {
        match err {
            AcmError::IoError(e) => e,
            other => std::io::Error::other(other),
        }
    }
}

// ─── Public info struct ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
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
    /// Total number of PCM words (samples × channels) in the stream.
    pub total_values: u32,
}

impl AcmInfo {
    /// Returns the number of PCM samples in the stream.
    pub fn samples(&self) -> u32 {
        self.total_values / self.channels
    }

    /// Returns the number of bits per PCM sample.
    /// This is always 16.
    pub fn bits_per_sample(&self) -> u16 {
        16
    }
}

// ─── Decoder ─────────────────────────────────────────────────────────────────

/// Output channel mode
#[derive(Debug, Copy, Clone)]
pub enum OutputChannels {
    /// Force mono output
    Mono,
    /// Force stereo output
    Stereo,
    /// Use the channel count from the original header
    Original,
}

pub struct AcmDecoder {
    pub info: AcmInfo,

    /// Caller-supplied label (resource name, file path, …) prefixed to log
    /// records so consumers decoding many streams can tell entries apart.
    name: String,

    // ── bitstream state ──
    /// Reader that pulls bytes from the source on demand, mirroring GemRB's
    /// ACMReader plugin: only the small bitstream window is held in memory
    /// at any time. The boxed `dyn BufRead` is fully owned (file handle for
    /// path/generator sources, cloned `Arc<Vec<u8>>` cursor for in-memory
    /// sources) so the decoder doesn't borrow from the original `DataSource`.
    reader: Reader<Box<dyn DataTrait>>,
    /// True once we have already returned the one padding word at EOF (matching
    /// the C libacm behaviour of appending 4 zero bytes to the raw data).
    eof_padding_given: bool,
    /// Bitstream register. The 64-bit width lets `get_bits` stay on the
    /// fast path twice as long as a 32-bit register would on a typical
    /// 1-7-bit-per-call workload, halving the slow-path frequency.
    bit_data: u64,
    bit_avail: u32,

    // ── block buffers ──
    /// Decoded block samples. `Box<[i32]>` (not `Vec`) because the
    /// length is fixed once `init` finishes — no `push`/`resize` after.
    block: Box<[i32]>,
    /// Stateful "edge" samples carried by the inverse-lifting transform
    /// across blocks. Length fixed at `init` time.
    wrapbuf: Box<[i32]>,
    /// 0x10000-element amplitude lookup table. `midbuf[i]` is
    /// `ampbuf[MIDBUF_OFFSET + i]`. Always exactly 64 K entries —
    /// `pwr=15` worst case fills all of `[MIDBUF_OFFSET-32768,
    /// MIDBUF_OFFSET+32768)`.
    ampbuf: Box<[i32]>,
    block_len: usize,

    block_ready: bool,
    wavc_file: bool,
    stream_pos: u32,
    block_pos: usize,

    // ── reset state ──
    /// Cloned source held so [`reset`](Self::reset) can reopen the stream
    /// without the caller having to thread the `DataSource` back in.
    /// `DataSource::clone` is cheap: file/generator paths just clone a
    /// `PathBuf`/`Arc`, in-memory sources clone an `Arc<Vec<u8>>`.
    datasource: DataSource,
    output_channels: OutputChannels,
}

impl std::fmt::Debug for AcmDecoder {
    /// Hand-written so the giant scratch buffers (`ampbuf` is 64 K entries,
    /// `block` and `wrapbuf` scale with `acm_level`) and the un-`Debug`
    /// `Box<dyn BufRead + Send>` reader stay out of the output. We surface the
    /// header, decode position, and bitstream state, which is what actually
    /// matters when logging or panicking.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcmDecoder")
            .field("name", &self.name)
            .field("info", &self.info)
            .field("output_channels", &self.output_channels)
            .field("stream_pos", &self.stream_pos)
            .field("total_values", &self.info.total_values)
            .field("block_len", &self.block_len)
            .field("block_pos", &self.block_pos)
            .field("block_ready", &self.block_ready)
            .field("bit_avail", &self.bit_avail)
            .field("eof_padding_given", &self.eof_padding_given)
            .field("wavc_file", &self.wavc_file)
            .finish_non_exhaustive()
    }
}

impl Clone for AcmDecoder {
    /// Cloning re-opens the stream from the start: the decoder holds live
    /// bitstream/block state and a boxed `dyn` reader that cannot be
    /// duplicated, so we reconstruct a fresh decoder from the retained
    /// [`DataSource`] (rewound to the beginning) instead.
    ///
    /// # Panics
    /// Panics if the source can no longer be opened — it was opened
    /// successfully once, so this only fires if it has since disappeared.
    fn clone(&self) -> Self {
        AcmDecoder::open(&self.datasource, self.output_channels, self.name.clone())
            .expect("AcmDecoder::clone: reopening the datasource failed")
    }
}

impl AcmDecoder {
    /// Open an ACM stream from a [`DataSource`]. The decoder pulls bytes
    /// lazily — block by block, like GemRB's ACMReader plugin — so memory
    /// use stays bounded regardless of the file size.
    ///
    /// `name` is a caller-supplied label (resource id, file path, …) that
    /// gets prefixed to every log record this decoder emits, so callers
    /// decoding many streams concurrently can tell entries apart.
    pub fn open(
        datasource: &DataSource,
        output_channels: OutputChannels,
        name: impl Into<String>,
    ) -> Result<Self> {
        // Construct with placeholder buffers; `init` opens the reader, parses
        // the header, and sizes everything correctly.
        let mut dec = AcmDecoder {
            info: AcmInfo::default(),
            name: name.into(),
            reader: datasource.reader()?,
            eof_padding_given: false,
            bit_data: 0,
            bit_avail: 0,
            block: Box::default(),
            wrapbuf: Box::default(),
            ampbuf: vec![0i32; 0x10000].into_boxed_slice(),
            block_len: 0,
            block_ready: false,
            wavc_file: false,
            stream_pos: 0,
            block_pos: 0,
            datasource: datasource.clone(),
            output_channels,
        };

        dec.init()?;
        Ok(dec)
    }

    /// Caller-supplied label passed at [`open`](Self::open) time — useful
    /// for logging or surfacing in a UI.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rewind the decoder to the start of the stream.
    ///
    /// Reopens the original [`DataSource`], re-reads the ACM header, and
    /// clears all bitstream / block state. After calling this, the next
    /// [`read_samples`](Self::read_samples) / [`decode_all`](Self::decode_all)
    /// / [`decode_to_file`](Self::decode_to_file) call produces the same
    /// output as the very first one — useful for looping playback or for
    /// retrying a partial decode.
    ///
    /// The previously chosen [`OutputChannels`] override and `name` are
    /// preserved.
    pub fn reset(&mut self) -> Result<()> {
        self.init()
    }

    /// Open the reader, read the header, and (re)allocate scratch buffers.
    /// Shared by [`open`](Self::open) and [`reset`](Self::reset).
    fn init(&mut self) -> Result<()> {
        self.reader = self.datasource.reader()?;

        // Bitstream state — must start empty so the first `get_bits` reads
        // a fresh word from the reopened reader.
        self.bit_data = 0;
        self.bit_avail = 0;
        self.eof_padding_given = false;

        // Decode position and per-block flags.
        self.block_ready = false;
        self.wavc_file = false;
        self.stream_pos = 0;
        self.block_pos = 0;
        self.block_len = 0;

        // Header is fully overwritten by `read_header`, but re-zero so a
        // partially populated `info` from a failed reset can't be observed.
        self.info = AcmInfo::default();

        self.read_header()?;

        match self.output_channels {
            OutputChannels::Mono => self.info.channels = 1,
            OutputChannels::Stereo => self.info.channels = 2,
            OutputChannels::Original => (),
        }

        // Compute derived dimensions.
        self.info.acm_cols = 1 << self.info.acm_level;
        let wrapbuf_len = (2 * self.info.acm_cols as usize).saturating_sub(2);
        self.block_len = self.info.acm_rows as usize * self.info.acm_cols as usize;

        // Allocate fresh buffers at the new size. `wrapbuf` is stateful
        // across blocks (juggle's edge samples), so leftover values from
        // a previous run would contaminate the first few output samples
        // on a reset — the new boxed slices start zeroed.
        self.block = vec![0i32; self.block_len].into_boxed_slice();
        self.wrapbuf = vec![0i32; wrapbuf_len.max(1)].into_boxed_slice();

        debug!(
            "[{}] Opened ACM: channels={}, rate={}, total_values={}",
            self.name, self.info.channels, self.info.rate, self.info.total_values
        );
        Ok(())
    }

    // ── bitstream helpers ────────────────────────────────────────────────────

    /// Load the next up-to-8 bytes from the reader as a little-endian
    /// word. Returns `(word, bits_loaded)`.
    ///
    /// At EOF, returns one synthetic zero word of **32 bits** before
    /// returning `(0, 0)`, mirroring the C libacm convention of
    /// appending exactly 4 zero bytes to the raw data — keeping this
    /// at 32 bits is what makes the byte-exact round-trip pass.
    fn load_next_word(&mut self) -> (u64, u32) {
        match self.reader.read_at_most_to_array::<8>() {
            Ok((buf, n)) if n > 0 => {
                let mut word = 0u64;
                for (i, byte) in buf.iter().enumerate().take(n) {
                    word |= (*byte as u64) << (i * 8);
                }
                (word, (n * 8) as u32)
            }
            _ => {
                // EOF or IO error: emit one zero padding word, then nothing.
                if !self.eof_padding_given {
                    self.eof_padding_given = true;
                    (0, 32)
                } else {
                    (0, 0)
                }
            }
        }
    }

    /// Extract `bits` bits from the bitstream (bits must be ≤ 32 — the
    /// callers actually max out at 16).
    #[inline]
    pub(crate) fn get_bits(&mut self, bits: u32) -> Result<u32> {
        if self.bit_avail >= bits {
            let mask = (1u64 << bits) - 1;
            let val = (self.bit_data & mask) as u32;
            self.bit_data >>= bits;
            self.bit_avail -= bits;
            return Ok(val);
        }
        self.get_bits_slow(bits)
    }

    /// Slow path: reload the bit register and extract.
    #[cold]
    #[inline(never)]
    fn get_bits_slow(&mut self, bits: u32) -> Result<u32> {
        let saved = self.bit_data; // bits already available
        let got = self.bit_avail;
        let remaining = bits - got;

        let (b_data, b_avail) = self.load_next_word();

        if b_avail < remaining {
            return Err(AcmError::UnexpectedEofError);
        }

        let mask = (1u64 << remaining) - 1;
        let val = (saved | ((b_data & mask) << got)) as u32;
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
            return Err(AcmError::NotAcmError);
        }
        // buf[6] must be 28 (header length magic)
        if buf[6] != 28 {
            return Err(AcmError::NotAcmError);
        }
        self.wavc_file = true;
        Ok(())
    }

    fn read_header(&mut self) -> Result<()> {
        let mut tmp = self.get_bits(24)?;

        if tmp == WAVC_ID {
            // WAVC variant: 'WAVC' = 0x43564157
            if self.get_bits(8)? != b'C' as u32 {
                return Err(AcmError::NotAcmError);
            }
            self.read_wavc_header()?;
            tmp = self.get_bits(24)?;
        }

        if tmp != ACM_ID {
            error!(
                "[{}] Not an ACM file: expected ID {ACM_ID:#x}, got {tmp:#x}",
                self.name
            );
            return Err(AcmError::NotAcmError);
        }
        self.info.acm_id = tmp;

        self.info.acm_version = self.get_bits(8)?;
        if self.info.acm_version != 1 {
            return Err(AcmError::NotAcmError);
        }

        // total_values stored as two 16-bit halves (lo, hi)
        let lo = self.get_bits(16)?;
        let hi = self.get_bits(16)?;
        self.info.total_values = lo | (hi << 16);
        if self.info.total_values == 0 {
            return Err(AcmError::NotAcmError);
        }

        self.info.channels = self.get_bits(16)?;
        if !(1..=2).contains(&self.info.channels) {
            return Err(AcmError::NotAcmError);
        }
        self.info.acm_channels = self.info.channels;

        self.info.rate = self.get_bits(16)?;
        if self.info.rate < 4096 {
            return Err(AcmError::NotAcmError);
        }

        self.info.acm_level = self.get_bits(4)?;
        self.info.acm_rows = self.get_bits(12)?;
        if self.info.acm_rows == 0 {
            return Err(AcmError::NotAcmError);
        }

        Ok(())
    }

    // ── amplitude table helper ───────────────────────────────────────────────

    /// Write `block[row * acm_cols + col] = ampbuf[MIDBUF_OFFSET + idx]`.
    #[inline]
    fn set_pos(&mut self, row: usize, col: usize, idx: i32) {
        let block_idx = (row << self.info.acm_level as usize) + col;
        let amp_idx = (MIDBUF_OFFSET as i32 + idx) as usize;
        self.block[block_idx] = self.ampbuf[amp_idx];
    }

    /// Read the variable-bit "tail" of one of the Huffman fillers and
    /// return the signed amplitude index it codes for.
    #[inline]
    fn read_huffman_tail(&mut self, tail: HuffTail) -> Result<i32> {
        Ok(match tail {
            HuffTail::M1 => MAP_1BIT[self.get_bits(1)? as usize],
            HuffTail::M2Near => MAP_2BIT_NEAR[self.get_bits(2)? as usize],
            HuffTail::M3 => MAP_3BIT[self.get_bits(3)? as usize],
            HuffTail::M1OrM2Far => {
                if self.get_bits(1)? == 0 {
                    MAP_1BIT[self.get_bits(1)? as usize]
                } else {
                    MAP_2BIT_FAR[self.get_bits(2)? as usize]
                }
            }
        })
    }

    /// Three-prefix Huffman skeleton — `0→(zero,zero) | 10→zero | 11{tail}`.
    /// Used by `f_k13` (17), `f_k24` (20), `f_k35` (23), `f_k45` (26).
    fn fill_huffman_zz_z(&mut self, col: usize, rows: usize, tail: HuffTail) -> Result<()> {
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
            let v = self.read_huffman_tail(tail)?;
            self.set_pos(i, col, v);
            i += 1;
        }
        Ok(())
    }

    /// Two-prefix Huffman skeleton — `0→zero | 1{tail}`. Used by
    /// `f_k12` (18), `f_k23` (21), `f_k34` (24), `f_k44` (27).
    fn fill_huffman_z(&mut self, col: usize, rows: usize, tail: HuffTail) -> Result<()> {
        for row in 0..rows {
            if self.get_bits(1)? == 0 {
                self.set_pos(row, col, 0);
            } else {
                let v = self.read_huffman_tail(tail)?;
                self.set_pos(row, col, v);
            }
        }
        Ok(())
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
            Err(AcmError::UnexpectedEofError) => return Ok(false),
            Err(e) => return Err(e),
        };
        let val = match self.get_bits(16) {
            Ok(v) => v as i32,
            Err(AcmError::UnexpectedEofError) => return Ok(false),
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
            Err(AcmError::UnexpectedEofError) => return Ok(false),
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
            1 | 2 | 25 | 28 | 30 | 31 => {
                error!("[{}] ACM corrupt: invalid filler index {}", self.name, ind);
                return Err(AcmError::CorruptError);
            }

            // ── f_linear (ind = 3..=16) ─────────────────────────────────────
            3..=16 => {
                let middle = 1i32 << (ind - 1);
                for row in 0..rows {
                    let b = self.get_bits(ind as u32)? as i32;
                    self.set_pos(row, col, b - middle);
                }
            }

            // ── f_k13: Huffman 0→(zero,zero) | 10→zero | 11?→±1 ─────────────
            17 => self.fill_huffman_zz_z(col, rows, HuffTail::M1)?,

            // ── f_k12: Huffman 0→zero | 1?→±1 ───────────────────────────────
            18 => self.fill_huffman_z(col, rows, HuffTail::M1)?,

            // ── f_t15 ───────────────────────────────────────────────────────
            // 5 bits encode 3 ternary values: v = x1 + x2*3 + x3*9
            19 => {
                let mut i = 0;
                while i < rows {
                    let b = self.get_bits(5)?;
                    if b >= 27 {
                        return Err(AcmError::CorruptError);
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

            // ── f_k24: Huffman 0→(zero,zero) | 10→zero | 11??→±1,±2 ─────────
            20 => self.fill_huffman_zz_z(col, rows, HuffTail::M2Near)?,

            // ── f_k23: Huffman 0→zero | 1??→±1,±2 ───────────────────────────
            21 => self.fill_huffman_z(col, rows, HuffTail::M2Near)?,

            // ── f_t27 ───────────────────────────────────────────────────────
            // 7 bits encode 3 quinary values: v = x1 + x2*5 + x3*25
            22 => {
                let mut i = 0;
                while i < rows {
                    let b = self.get_bits(7)?;
                    if b >= 125 {
                        return Err(AcmError::CorruptError);
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

            // ── f_k35: Huffman 0→(zero,zero) | 10→zero | 110?→±1 | 111??→±2,±3
            23 => self.fill_huffman_zz_z(col, rows, HuffTail::M1OrM2Far)?,

            // ── f_k34: Huffman 0→zero | 10?→±1 | 11??→±2,±3 ─────────────────
            24 => self.fill_huffman_z(col, rows, HuffTail::M1OrM2Far)?,

            // ── f_k45: Huffman 0→(zero,zero) | 10→zero | 11???→±1..±4 ───────
            26 => self.fill_huffman_zz_z(col, rows, HuffTail::M3)?,

            // ── f_k44: Huffman 0→zero | 1???→±1..±4 ─────────────────────────
            27 => self.fill_huffman_z(col, rows, HuffTail::M3)?,

            // ── f_t37 ───────────────────────────────────────────────────────
            // 7 bits encode 2 values in base-11: v = x1 + x2*11
            29 => {
                let mut i = 0;
                while i < rows {
                    let b = self.get_bits(7)?;
                    if b >= 121 {
                        return Err(AcmError::CorruptError);
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

            _ => return Err(AcmError::CorruptError),
        }

        Ok(())
    }

    // ── public decode API ────────────────────────────────────────────────────

    /// Decode the next chunk of PCM samples into `out`, returning the number
    /// of `i16` samples written. Returns `Ok(0)` only on natural end of stream.
    ///
    /// This is the GemRB-style streaming hook: an audio player calls it
    /// repeatedly, refilling its mixer buffer one block at a time, without
    /// ever holding the entire decoded waveform in memory.
    ///
    /// Samples are interleaved (frame-major) for stereo streams; on stereo,
    /// `out.len()` should be at least `info.channels` (i.e. ≥ 2) so a frame
    /// boundary can be honoured — otherwise the call returns 0 to avoid
    /// splitting a stereo pair across two reads.
    pub fn read_samples(&mut self, out: &mut [i16]) -> Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let shift = self.info.acm_level;
        let channels = self.info.channels as usize;
        let mut written = 0usize;

        while written < out.len() && self.stream_pos < self.info.total_values {
            if !self.block_ready && !self.decode_block()? {
                break; // natural EOF
            }

            let avail = self.block_len - self.block_pos;
            let remaining = (self.info.total_values - self.stream_pos) as usize;
            let buf_remaining = out.len() - written;
            let mut n = avail.min(remaining).min(buf_remaining);

            // Keep stereo pairs aligned both in the output buffer and across
            // block boundaries, matching GemRB's read_samples behaviour.
            if channels > 1 {
                n -= n % channels;
            }

            if n == 0 {
                // Either the block ran out on an odd boundary, or the
                // caller's remaining buffer can't fit a full frame. If
                // there's no room left in the buffer, stop; otherwise
                // discard the trailing odd sample and try the next block.
                if buf_remaining < channels.max(1) {
                    break;
                }
                self.block_ready = false;
                continue;
            }

            for i in 0..n {
                let v = self.block[self.block_pos + i] >> shift;
                out[written + i] = v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            }

            self.stream_pos += n as u32;
            self.block_pos += n;
            written += n;
            if self.block_pos == self.block_len {
                self.block_ready = false;
            }
        }

        Ok(written)
    }

    /// Number of PCM samples already produced by [`read_samples`] /
    /// [`decode_all`] since the decoder was opened. Useful for an audio
    /// player that wants to compute current playback position without
    /// tracking it externally.
    pub fn samples_decoded(&self) -> u32 {
        self.stream_pos
    }

    /// Decode the entire ACM stream and return all PCM samples as signed 16-bit
    /// values in interleaved channel order (same as s16le WAV data).
    ///
    /// Equivalent to repeatedly calling [`read_samples`] into a
    /// `info.total_values`-sized buffer; trailing samples that the stream
    /// doesn't actually produce are left as zeros (matching the libacm WAV
    /// tool's behaviour of zero-filling to the declared sample count).
    pub fn decode_all(&mut self) -> Result<Vec<i16>> {
        let total = self.info.total_values as usize;
        let mut samples = vec![0i16; total];
        let mut written = 0usize;
        while written < total {
            let n = self.read_samples(&mut samples[written..])?;
            if n == 0 {
                break;
            }
            written += n;
        }
        Ok(samples)
    }

    /// Decode the ACM stream into a WAV file with signed 16-bit PCM samples
    /// in interleaved channel order (s16le).
    ///
    /// Streams block-by-block through [`read_samples`] straight into the WAV
    /// writer, so peak memory stays at the size of one decoded block plus the
    /// scratch buffer regardless of file size. Trailing samples that the
    /// stream doesn't actually produce are zero-padded to `info.total_values`,
    /// matching the libacm tool's WAV output.
    pub fn decode_to_file<P: AsRef<std::path::Path>>(&mut self, filename: P) -> Result<()> {
        let spec = hound::WavSpec {
            channels: self.info.channels as u16,
            sample_rate: self.info.rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(filename, spec)?;
        let total = self.info.total_values as usize;
        let mut buf = [0i16; 4096];
        let mut written = 0usize;

        while written < total {
            // Cap the buffer so we never request more than the declared total.
            let want = (total - written).min(buf.len());
            let n = self.read_samples(&mut buf[..want])?;
            if n == 0 {
                break; // natural EOF before total_values — fall through to padding
            }
            for &s in &buf[..n] {
                writer.write_sample(s)?;
            }
            written += n;
        }

        // Zero-pad up to the declared total, matching libacm's tool.
        for _ in written..total {
            writer.write_sample(0i16)?;
        }

        writer.finalize()?;
        Ok(())
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
