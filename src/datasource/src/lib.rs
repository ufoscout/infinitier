#![doc = include_str!("../readme.md")]

use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek, Take},
    path::{Path, PathBuf},
    sync::Arc,
};

use encoding_rs::{Encoding, WINDOWS_1252};
use flate2::bufread::ZlibDecoder;

mod generator;
pub use generator::TempFileGenerator;

/// A data importer.
/// Parses data from a data source and returns the parsed data
pub trait Importer {
    type T;
    /// Imports a data source
    fn import(&self, source: &DataSource) -> std::io::Result<Self::T>;
}

/// A data source
#[derive(Debug, Clone)]
pub enum Data {
    Path(PathBuf),
    Generator(Arc<TempFileGenerator>),
    MemorySource(Arc<Vec<u8>>),
}

impl From<PathBuf> for Data {
    fn from(value: PathBuf) -> Self {
        Data::Path(value)
    }
}

impl From<&Path> for Data {
    fn from(value: &Path) -> Self {
        Data::Path(value.to_path_buf())
    }
}

impl From<&str> for Data {
    fn from(value: &str) -> Self {
        Data::Path(PathBuf::from(value))
    }
}

impl From<Vec<u8>> for Data {
    fn from(value: Vec<u8>) -> Self {
        Data::MemorySource(Arc::new(value))
    }
}

impl From<Arc<Vec<u8>>> for Data {
    fn from(value: Arc<Vec<u8>>) -> Self {
        Data::MemorySource(value)
    }
}

impl From<&[u8]> for Data {
    fn from(value: &[u8]) -> Self {
        Data::MemorySource(Arc::new(value.to_vec()))
    }
}

impl<const N: usize> From<&[u8; N]> for Data {
    fn from(value: &[u8; N]) -> Self {
        Data::MemorySource(Arc::new(value.to_vec()))
    }
}

pub trait DataTrait: Read + BufRead + Seek + Send + Sync {}

impl DataTrait for BufReader<File> {}
impl DataTrait for Cursor<&[u8]> {}
impl DataTrait for Cursor<SharedBytes> {}
impl<D: DataTrait> DataTrait for Take<D> {}

impl Data {
    /// Byte length of the data, computed without reading the content
    /// into memory. For [`Data::Path`] this is a single `stat` call; for
    /// [`Data::Generator`] it realises the temp file (if necessary) and
    /// stats it; for [`Data::MemorySource`] it's `Vec::len`.
    ///
    /// Surfacing this allows callers — notably the [`DataSource::Concat`]
    /// reader — to compute cumulative offsets across multiple parts
    /// without first reading their bytes.
    pub fn len(&self) -> std::io::Result<u64> {
        match self {
            Data::Path(path) => Ok(std::fs::metadata(path)?.len()),
            Data::Generator(generator) => Ok(std::fs::metadata(generator.path()?)?.len()),
            Data::MemorySource(bytes) => Ok(bytes.len() as u64),
        }
    }

    /// `true` when `len()` is `0`. Provided to satisfy clippy's
    /// `len_without_is_empty` lint.
    pub fn is_empty(&self) -> std::io::Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Returns a reader for the data
    pub fn reader(&self, offset: u64, limit: Option<u64>) -> std::io::Result<Box<dyn DataTrait>> {
        match self {
            Data::Path(reader) => {
                let mut data = BufReader::new(File::open(reader)?);
                data.seek(std::io::SeekFrom::Start(offset))?;
                if let Some(limit) = limit {
                    Ok(Box::new(data.take(limit)))
                } else {
                    Ok(Box::new(data))
                }
            }
            Data::Generator(generator) => {
                let mut data = BufReader::new(File::open(generator.path()?)?);
                data.seek(std::io::SeekFrom::Start(offset))?;
                if let Some(limit) = limit {
                    Ok(Box::new(data.take(limit)))
                } else {
                    Ok(Box::new(data))
                }
            }
            Data::MemorySource(reader) => {
                let mut data = Cursor::new(SharedBytes(reader.clone()));
                data.seek(std::io::SeekFrom::Start(offset))?;
                if let Some(limit) = limit {
                    Ok(Box::new(data.take(limit)))
                } else {
                    Ok(Box::new(data))
                }
            }
        }
    }
}

/// `Arc<Vec<u8>>` doesn't impl `AsRef<[u8]>`, so wrap it for `Cursor`.
struct SharedBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// [`DataTrait`] adapter that reads from several [`DataSource`]s in
/// sequence as if they were one stream — backs [`DataSource::Concat`].
///
/// **Lazy by construction**: at most one part is open at a time, and a
/// part is opened only when a read or seek actually lands inside it. No
/// part's bytes are pulled into memory ahead of time; segment lengths
/// (needed for seeking) come from [`Data::len`], which only does a
/// `stat`-like metadata lookup.
struct ConcatReader {
    /// Source parts, cloned at construction so the reader can outlive
    /// the borrowed slice it came from. `DataSource` is cheap to clone
    /// — file paths and `Arc`-shared byte buffers.
    parts: Vec<DataSource>,
    /// Cumulative byte offsets: `segment_starts[i]` = sum of lengths of
    /// `parts[..i]`. The last entry is the total concatenated length.
    /// Computed once at construction via stat / `Vec::len`.
    segment_starts: Vec<u64>,
    /// Currently-open part: `(part_index, opened_reader)`. `None` until
    /// the first read/seek lands inside a part. Switching parts drops
    /// the previous reader, so we keep at most one file handle open.
    current: Option<(usize, Box<dyn DataTrait>)>,
    /// Global cursor position, in `[0, total_length]`.
    position: u64,
}

impl ConcatReader {
    fn new(parts: &[DataSource]) -> std::io::Result<Self> {
        let mut segment_starts = Vec::with_capacity(parts.len() + 1);
        let mut total = 0u64;
        segment_starts.push(0);
        for part in parts {
            total = total.saturating_add(part.len()?);
            segment_starts.push(total);
        }
        Ok(Self {
            parts: parts.to_vec(),
            segment_starts,
            current: None,
            position: 0,
        })
    }

    fn total_length(&self) -> u64 {
        *self.segment_starts.last().unwrap_or(&0)
    }

    /// Index of the part that contains the byte at `pos`. When `pos`
    /// falls exactly on a part boundary, returns the part that *starts*
    /// at that offset (the later one). Returns `parts.len()` when
    /// `pos >= total_length`.
    fn segment_for_position(&self, pos: u64) -> usize {
        if pos >= self.total_length() {
            return self.parts.len();
        }
        // segment_starts is sorted, with parts.len() + 1 entries. Find
        // the largest i with segment_starts[i] <= pos.
        match self.segment_starts.binary_search(&pos) {
            Ok(i) => i.min(self.parts.len()), // exact match = boundary
            Err(i) => i.saturating_sub(1),    // pos is between i-1 and i
        }
    }

    /// Make sure `self.current` is open for the part containing
    /// `self.position`, with the underlying reader seeked to the
    /// corresponding local offset. No-op when already aligned.
    /// Caller must check `position < total_length` first.
    fn ensure_open(&mut self) -> std::io::Result<()> {
        let seg = self.segment_for_position(self.position);
        debug_assert!(seg < self.parts.len());
        let local = self.position - self.segment_starts[seg];

        let needs_open = match &self.current {
            Some((i, _)) => *i != seg,
            None => true,
        };
        if needs_open {
            // Drop the old reader first so we never hold two file
            // handles at once.
            self.current = None;
            let mut reader = self.parts[seg].data_reader()?;
            if local > 0 {
                reader.seek(std::io::SeekFrom::Start(local))?;
            }
            self.current = Some((seg, reader));
        }
        Ok(())
    }

    /// Step past every empty part starting at the current position so
    /// `self.position` lands either on a non-empty part or at EOF.
    /// Zero-length parts are surprisingly easy to produce (a `Concat`
    /// with no parts, a `Path` to an empty file, …) — without this the
    /// reader would loop forever on them in `read` / `fill_buf`.
    fn skip_empty_parts(&mut self) {
        // segment_starts[i] == segment_starts[i+1] means part i is empty.
        while self.position < self.total_length() {
            let seg = self.segment_for_position(self.position);
            if self.segment_starts[seg] == self.segment_starts[seg + 1] {
                self.position = self.segment_starts[seg + 1];
                self.current = None;
            } else {
                break;
            }
        }
    }
}

impl Read for ConcatReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            self.skip_empty_parts();
            if self.position >= self.total_length() {
                return Ok(0);
            }
            self.ensure_open()?;
            let n = self.current.as_mut().unwrap().1.read(buf)?;
            if n > 0 {
                self.position += n as u64;
                return Ok(n);
            }
            // Underlying reader returned 0 but we're not at total EOF
            // → this part is exhausted earlier than its declared length
            // (e.g. the backing file was truncated since `len()` was
            // computed). Jump to the next part's start and try again.
            let seg = self.segment_for_position(self.position);
            self.position = self.segment_starts[seg + 1];
            self.current = None;
        }
    }
}

impl BufRead for ConcatReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        loop {
            self.skip_empty_parts();
            if self.position >= self.total_length() {
                return Ok(&[]);
            }
            self.ensure_open()?;
            // Two-step dance to keep the borrow checker happy: the
            // first call lets us inspect whether the buffer is empty
            // without keeping the borrow alive across `self.position`
            // mutation. The second call returns the same buffer (every
            // `BufRead` impl caches its result, so this is essentially
            // free).
            let is_empty = self.current.as_mut().unwrap().1.fill_buf()?.is_empty();
            if !is_empty {
                return self.current.as_mut().unwrap().1.fill_buf();
            }
            // Current part returned an empty buffer despite us not
            // having reached its declared end — same truncation case
            // as in `read`. Advance to the next part.
            let seg = self.segment_for_position(self.position);
            self.position = self.segment_starts[seg + 1];
            self.current = None;
        }
    }

    fn consume(&mut self, amt: usize) {
        if amt == 0 {
            return;
        }
        if let Some((_, reader)) = self.current.as_mut() {
            reader.consume(amt);
        }
        self.position = self
            .position
            .saturating_add(amt as u64)
            .min(self.total_length());
    }
}

impl Seek for ConcatReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let total = self.total_length();
        let new_pos = match pos {
            std::io::SeekFrom::Start(n) => n,
            std::io::SeekFrom::End(off) => add_signed_clamp(total, off)?,
            std::io::SeekFrom::Current(off) => add_signed_clamp(self.position, off)?,
        };
        // Allow seeking exactly to total_length (end-of-stream marker);
        // any read from there will yield 0.
        if new_pos > total {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek past end of concatenated stream",
            ));
        }
        self.position = new_pos;
        // Either reseek the current part to the right local offset, or
        // drop it so `ensure_open` re-creates it on the next read.
        if new_pos < total {
            let seg = self.segment_for_position(new_pos);
            match &mut self.current {
                Some((i, reader)) if *i == seg => {
                    let local = new_pos - self.segment_starts[seg];
                    reader.seek(std::io::SeekFrom::Start(local))?;
                }
                _ => self.current = None,
            }
        } else {
            // Sitting exactly at total — no current part is meaningful.
            self.current = None;
        }
        Ok(self.position)
    }
}

impl DataTrait for ConcatReader {}

/// Compute `base + offset` for a signed offset, returning
/// `InvalidInput` on overflow / underflow rather than panicking.
fn add_signed_clamp(base: u64, offset: i64) -> std::io::Result<u64> {
    let result = if offset >= 0 {
        base.checked_add(offset as u64)
    } else {
        base.checked_sub((-(offset as i128)) as u64)
    };
    result.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek offset out of range")
    })
}

/// A data source with a specific encoding
#[derive(Debug, Clone)]
pub enum DataSource {
    Full {
        encoding: &'static Encoding,
        data: Data,
    },
    Embedded {
        encoding: &'static Encoding,
        data: Data,
        offset: u64,
        limit: Option<u64>,
    },
    /// Logical concatenation of multiple data sources.
    ///
    /// The reader returned by [`DataSource::reader`] walks the parts in
    /// order: when the current part is exhausted it transparently
    /// switches to the next one. Seeking is also transparent — the
    /// reader maps a global offset to `(part_index, local_offset)` and
    /// only opens that one part. **No part is read into memory ahead of
    /// time**; each part's bytes are pulled from disk (or its in-memory
    /// `Vec`) on demand, and at most one part is kept open at a time.
    ///
    /// The optional `offset` / `limit` apply an outer window over the
    /// concatenated stream — same convention as [`DataSource::Embedded`]
    /// for non-concat data. Windowing stays lazy: it's implemented by
    /// seeking the [`ConcatReader`] and wrapping it in `Take`, not by
    /// materialising bytes.
    ///
    /// Built via [`DataSource::new_concat`] (default `offset = 0`,
    /// `limit = None`); window with [`DataSource::with_offset`].
    Concat {
        encoding: &'static Encoding,
        parts: Vec<DataSource>,
        offset: u64,
        limit: Option<u64>,
    },
}

impl From<Data> for DataSource {
    fn from(value: Data) -> Self {
        DataSource::new(value)
    }
}

impl DataSource {
    /// Creates a new data source
    pub fn new<D: Into<Data>>(data: D) -> Self {
        DataSource::Full {
            encoding: WINDOWS_1252,
            data: data.into(),
        }
    }

    /// Creates a new data source with an offset
    pub fn new_with_offset<D: Into<Data>>(data: D, offset: u64, limit: Option<u64>) -> Self {
        DataSource::Embedded {
            encoding: WINDOWS_1252,
            data: data.into(),
            offset,
            limit,
        }
    }

    /// Creates a data source that reads `parts` back-to-back as a single
    /// logical byte stream. See [`DataSource::Concat`] for the lazy /
    /// no-preload guarantee.
    pub fn new_concat(parts: Vec<DataSource>) -> Self {
        DataSource::Concat {
            encoding: WINDOWS_1252,
            parts,
            offset: 0,
            limit: None,
        }
    }

    /// Total byte length of the data source, computed without reading
    /// the content. For [`DataSource::Concat`] this is the sum of every
    /// part's length (each computed via [`Data::len`]) clamped by the
    /// outer `offset` / `limit` window; for [`DataSource::Embedded`]
    /// it accounts for the offset and limit.
    pub fn len(&self) -> std::io::Result<u64> {
        match self {
            DataSource::Full { data, .. } => data.len(),
            DataSource::Embedded {
                data,
                offset,
                limit,
                ..
            } => {
                let after_offset = data.len()?.saturating_sub(*offset);
                Ok(match limit {
                    Some(l) => after_offset.min(*l),
                    None => after_offset,
                })
            }
            DataSource::Concat {
                parts,
                offset,
                limit,
                ..
            } => {
                let mut total = 0u64;
                for p in parts {
                    total = total.saturating_add(p.len()?);
                }
                let after_offset = total.saturating_sub(*offset);
                Ok(match limit {
                    Some(l) => after_offset.min(*l),
                    None => after_offset,
                })
            }
        }
    }

    /// `true` when [`DataSource::len`] is `0`.
    pub fn is_empty(&self) -> std::io::Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Sets the encoding
    pub fn with_encoding(self, encoding: &'static Encoding) -> Self {
        match self {
            DataSource::Full { data, .. } => DataSource::Full { encoding, data },
            DataSource::Embedded {
                data,
                offset,
                limit,
                ..
            } => DataSource::Embedded {
                encoding,
                data,
                offset,
                limit,
            },
            DataSource::Concat {
                parts,
                offset,
                limit,
                ..
            } => DataSource::Concat {
                encoding,
                parts,
                offset,
                limit,
            },
        }
    }

    /// Applies an `(offset, limit)` window to the data source.
    ///
    /// The new window **replaces** any existing one (same semantics as
    /// [`DataSource::Embedded`] — `with_offset` is not compositional).
    ///
    /// For [`DataSource::Concat`] the window is applied to the
    /// concatenated logical stream: `offset` skips that many bytes
    /// across however many parts they span, and `limit` caps the
    /// total visible length. The concatenation stays lazy — windowing
    /// is implemented at read time by seeking the underlying
    /// `ConcatReader` and wrapping it in [`std::io::Read::take`], not
    /// by materialising bytes.
    pub fn with_offset(self, offset: u64, limit: Option<u64>) -> Self {
        match self {
            DataSource::Full { encoding, data } => DataSource::Embedded {
                encoding,
                data,
                offset,
                limit,
            },
            DataSource::Embedded { encoding, data, .. } => DataSource::Embedded {
                encoding,
                data,
                offset,
                limit,
            },
            DataSource::Concat {
                encoding, parts, ..
            } => DataSource::Concat {
                encoding,
                parts,
                offset,
                limit,
            },
        }
    }

    /// Returns the encoding
    pub fn encoding(&self) -> &'static Encoding {
        match self {
            DataSource::Full { encoding, .. } => encoding,
            DataSource::Embedded { encoding, .. } => encoding,
            DataSource::Concat { encoding, .. } => encoding,
        }
    }

    /// Creates a data reader
    pub fn reader(&self) -> std::io::Result<Reader<Box<dyn DataTrait>>> {
        Ok(Reader {
            data: self.data_reader()?,
            charset: self.encoding(),
        })
    }

    /// Like [`Self::reader`], but eagerly reads the entire source
    /// into a `Vec<u8>` so the returned [`Reader`] is backed by an
    /// in-memory [`Cursor`].
    pub fn preloaded_reader(&self) -> std::io::Result<Reader<std::io::Cursor<Vec<u8>>>> {
        let mut buf = Vec::new();
        self.reader()?.read_to_end(&mut buf)?;
        Ok(Reader {
            data: std::io::Cursor::new(buf),
            charset: self.encoding(),
        })
    }

    /// Internal: build just the [`DataTrait`] reader (no charset
    /// wrapping). Used by [`Self::reader`] and by [`ConcatReader`] when
    /// it opens a part lazily on demand.
    fn data_reader(&self) -> std::io::Result<Box<dyn DataTrait>> {
        match self {
            DataSource::Full { data, .. } => data.reader(0, None),
            DataSource::Embedded {
                data,
                offset,
                limit,
                ..
            } => data.reader(*offset, *limit),
            DataSource::Concat {
                parts,
                offset,
                limit,
                ..
            } => {
                // Build the unwindowed reader first, then apply the
                // outer window the same way `Data::reader` does for
                // path/memory sources: seek to `offset`, then wrap in
                // `Take(limit)` if a limit is set. `Take<T: Seek>`
                // (stable since 1.89) normalises its position to start
                // at 0, so the consumer sees the windowed stream the
                // same way as `DataSource::Embedded` does.
                let mut reader = ConcatReader::new(parts)?;
                if *offset > 0 {
                    reader.seek(std::io::SeekFrom::Start(*offset))?;
                }
                if let Some(l) = limit {
                    Ok(Box::new(reader.take(*l)))
                } else {
                    Ok(Box::new(reader))
                }
            }
        }
    }
}

/// A reader that reads a byte array with a specific encoding
pub struct Reader<T> {
    data: T,
    pub charset: &'static Encoding,
}

impl<T> Reader<T> {
    /// Creates a new reader
    pub fn new(data: T, charset: &'static Encoding) -> Self {
        Reader { data, charset }
    }

    /// with charset
    pub fn with_charset(mut self, charset: &'static Encoding) -> Self {
        self.charset = charset;
        self
    }

    /// set charset
    pub fn set_charset(&mut self, charset: &'static Encoding) {
        self.charset = charset;
    }
}

impl<T: Read> Reader<T> {
    /// Creates a Reader which will read at most limit bytes from it.
    pub fn take_as_reader(&mut self, bytes: u64) -> Reader<Take<&mut T>> {
        Reader {
            data: (&mut self.data).take(bytes),
            charset: self.charset,
        }
    }

    /// Read the first `n_chars` characters from a byte array interpreted
    /// with the Reader `charset`, and return them as a `String`.
    pub fn read_string(&mut self, size: u64) -> std::io::Result<String> {
        let buf = self.take_to_vec(size)?;
        let (decoded, _, had_errors) = self.charset.decode(&buf);

        if had_errors {
            return Err(std::io::Error::other(
                "Decoding error: input is not valid for this charset",
            ));
        }

        // Trim trailing null bytes at the end as the strings use the C string convention for null-termination
        Ok(decoded
            .chars()
            .collect::<String>()
            .trim_end_matches(char::from(0))
            .to_owned())
    }
}

impl<T: Read + Seek> Reader<T> {
    /// Reads a string from the offset position
    pub fn read_string_at(&mut self, offset: u64, size: u64) -> std::io::Result<String> {
        self.seek(std::io::SeekFrom::Start(offset))?;
        self.read_string(size)
    }
}

pub trait ReadExt: Read + Sized {
    /// Reads exactly `N` bytes from the current position and returns them as a byte array.
    ///
    /// If the end of the file is reached before `N` bytes could be read, an `io::Error` is returned.
    fn read_exact_to_array<const N: usize>(&mut self) -> std::io::Result<[u8; N]> {
        let mut buf = [0u8; N];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads up to `N` bytes from the current position and returns them as a tuple of a byte array and the number of bytes read.
    fn read_at_most_to_array<const N: usize>(&mut self) -> std::io::Result<([u8; N], usize)> {
        let mut buf = [0u8; N];
        let n = self.read(&mut buf)?;
        Ok((buf, n))
    }

    /// Reads up to `N` bytes from the current position and returns them as a `Vec<u8>`.
    ///
    /// If the end of the file is reached before `N` bytes could be read, the returned
    /// `Vec<u8>` will contain less than `N` elements.
    fn take_to_vec(&mut self, bytes: u64) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![];
        let mut chunk = (self).take(bytes);
        chunk.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Reads a i16 from the current position
    fn read_i16(&mut self) -> std::io::Result<i16> {
        Ok(i16::from_le_bytes(self.read_exact_to_array::<2>()?))
    }

    /// Reads a i32 from the current position
    fn read_i32(&mut self) -> std::io::Result<i32> {
        Ok(i32::from_le_bytes(self.read_exact_to_array::<4>()?))
    }

    /// Reads a u32 from the current position
    fn read_u32(&mut self) -> std::io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact_to_array::<4>()?))
    }

    /// Reads a u64 from the current position
    fn read_u64(&mut self) -> std::io::Result<u64> {
        Ok(u64::from_le_bytes(self.read_exact_to_array::<8>()?))
    }

    /// Reads a u16 from the current position
    fn read_u16(&mut self) -> std::io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact_to_array::<2>()?))
    }

    /// Reads a u8 from the current position
    #[inline]
    fn read_u8(&mut self) -> std::io::Result<u8> {
        Ok(u8::from_le_bytes(self.read_exact_to_array::<1>()?))
    }

    /// Reads a i8 from the current position
    fn read_i8(&mut self) -> std::io::Result<i8> {
        Ok(i8::from_le_bytes(self.read_exact_to_array::<1>()?))
    }

    /// Copy data from the reader to the writer
    fn copy(&mut self, writer: &mut impl std::io::Write) -> std::io::Result<u64> {
        std::io::copy(self, writer)
    }
}

impl<T: Read> ReadExt for T {}

pub trait SeekExt: Seek + Read + Sized {
    /// Returns the current position of the cursor.
    /// The position is relative to the Reader offset
    fn position(&mut self) -> std::io::Result<u64> {
        self.stream_position()
    }

    /// Sets the position of the cursor.
    /// The position is relative to the Reader offset.
    fn set_position(&mut self, pos: u64) -> std::io::Result<u64> {
        self.seek(std::io::SeekFrom::Start(pos)) //.map(|pos| pos - self.offset)
    }

    /// Reads a u32 from the offset position
    fn read_u32_at(&mut self, offset: u64) -> std::io::Result<u32> {
        self.set_position(offset)?;
        self.read_u32()
    }

    /// Reads a i32 from the offset position
    fn read_i32_at(&mut self, offset: u64) -> std::io::Result<i32> {
        self.set_position(offset)?;
        self.read_i32()
    }

    /// Reads a u16 from the offset position
    fn read_u16_at(&mut self, offset: u64) -> std::io::Result<u16> {
        self.set_position(offset)?;
        self.read_u16()
    }
}

impl<T: Seek + Read> SeekExt for T {}

impl<T: BufRead> Reader<T> {
    /// Returns a zip reader
    pub fn as_zip_reader(&mut self) -> Reader<ZlibDecoder<&mut T>> {
        Reader {
            data: ZlibDecoder::new(&mut self.data),
            charset: self.charset,
            // offset: self.offset,
        }
    }

    /// Reads a line from the current position
    /// and returns it as a `String` and the number of bytes read.
    /// If bytes read is 0, then EOF has been reached
    pub fn read_line(&mut self) -> std::io::Result<(String, usize)> {
        let mut buf = vec![];
        let bytes = self.data.read_until(b'\n', &mut buf)?;
        let (decoded, _, _) = self.charset.decode(&buf);
        Ok((decoded.into_owned(), bytes))
    }
}

impl<T: BufRead + Seek> Reader<T> {
    /// Reads a line from the offset position
    /// and returns it as a `String` and the number of bytes read.
    /// If bytes read is 0, then EOF has been reached
    pub fn read_line_at(&mut self, offset: u64) -> std::io::Result<(String, usize)> {
        self.data.seek(std::io::SeekFrom::Start(offset))?;
        self.read_line()
    }
}

impl<R: BufRead> Reader<ZlibDecoder<R>> {
    /// Skips `size` bytes and returns the number of bytes skipped.
    /// This operation has cost O(n), if the Reader is Seekable, use `seek` instead.
    pub fn skip(&mut self, size: u64) -> std::io::Result<u64> {
        std::io::copy(&mut (&mut self.data).take(size), &mut std::io::sink())
    }

    /// Decodes the entire zlib stream into memory
    pub fn decode_all(&mut self) -> std::io::Result<Reader<std::io::Cursor<Vec<u8>>>> {
        let mut data = Vec::new();
        self.data.read_to_end(&mut data)?;
        Ok(Reader {
            data: std::io::Cursor::new(data),
            charset: self.charset,
        })
    }
}

impl<T: Read> Read for Reader<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.data.read(buf)
    }

    /// Reads all bytes until EOF and appends them to `buf`, decoding through the
    /// reader's charset (which may not be UTF-8). Returns the number of bytes
    /// appended to `buf`. On decoding error, `buf` is left unchanged.
    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        let mut bytes = Vec::new();
        self.data.read_to_end(&mut bytes)?;
        let (decoded, _, had_errors) = self.charset.decode(&bytes);
        if had_errors {
            return Err(std::io::Error::other(
                "Decoding error: input is not valid for this charset",
            ));
        }
        let n = decoded.len();
        buf.push_str(&decoded);
        Ok(n)
    }
}

impl<T: BufRead> BufRead for Reader<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.data.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.data.consume(amount)
    }

    /// Reads bytes up to and including the next `\n`, decodes them through the
    /// reader's charset (which may not be UTF-8), and appends the result to
    /// `buf`. Returns the number of bytes appended to `buf`. On decoding error,
    /// `buf` is left unchanged.
    fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        let mut bytes = Vec::new();
        self.data.read_until(b'\n', &mut bytes)?;
        let (decoded, _, had_errors) = self.charset.decode(&bytes);
        if had_errors {
            return Err(std::io::Error::other(
                "Decoding error: input is not valid for this charset",
            ));
        }
        let n = decoded.len();
        buf.push_str(&decoded);
        Ok(n)
    }
}

impl<T: Seek> Seek for Reader<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.data.seek(pos)
    }
}

#[cfg(test)]
mod tests {

    use std::io::SeekFrom;

    use super::*;

    #[test]
    fn test_read_string() {
        let reader = DataSource::new("Hello, world!".as_bytes());
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_string(5).unwrap(), "Hello");
    }

    #[test]
    fn test_read_with_offset() {
        let reader =
            DataSource::new_with_offset("Hello, world! Hello, world!".as_bytes(), 5, Some(7));
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.position().unwrap(), 0);
        assert_eq!(reader.read_string(5).unwrap(), ", wor");
        assert_eq!(reader.position().unwrap(), 5);
        assert_eq!(reader.read_string(10).unwrap(), "ld");
        assert_eq!(reader.position().unwrap(), 7);
    }

    #[test]
    fn test_read_with_offset_and_position() {
        let reader =
            DataSource::new_with_offset("Hello, world! Hello, world!".as_bytes(), 5, Some(8));
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.set_position(3).unwrap(), 3);
        assert_eq!(reader.position().unwrap(), 3);
        assert_eq!(reader.read_string(5).unwrap(), "orld!");
    }

    #[test]
    fn test_read_to_end_should_respect_offset_and_limit() {
        let reader =
            DataSource::new_with_offset("Hello, world! Hello, world!".as_bytes(), 5, Some(7));
        let mut reader = reader.reader().unwrap();

        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();

        assert_eq!(String::from_utf8(buf).unwrap(), ", world".to_owned());
    }

    #[test]
    fn test_read_string_at() {
        let reader = DataSource::new("Hello, world!".as_bytes());
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_string_at(7, 5).unwrap(), "world");
    }

    #[test]
    fn test_read_u32() {
        let reader = DataSource::new(&[0x01u8, 0x02, 0x03, 0x04]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_u32().unwrap(), 0x04030201);
    }

    #[test]
    fn test_read_u32_at() {
        let reader = DataSource::new(&[0x01, 0x02, 0x01, 0x01, 0x03, 0x04]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_u32_at(2).unwrap(), 0x04030101);
    }

    #[test]
    fn test_read_i32() {
        let reader = DataSource::new(&[0x01, 0x02, 0x03, 0x04]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_i32().unwrap(), 0x04030201);
    }

    #[test]
    fn test_read_i32_at() {
        let reader = DataSource::new(&[0x01, 0x01, 0x01, 0x02, 0x01, 0x04]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_i32_at(2).unwrap(), 0x04010201);
    }

    #[test]
    fn test_read_u16() {
        let reader = DataSource::new(&[0x01, 0x02]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_u16().unwrap(), 0x0201);
    }

    #[test]
    fn test_read_u16_at() {
        let reader = DataSource::new(&[0x01, 0x02, 0x03, 0x04]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_u16_at(2).unwrap(), 0x0403);
    }

    #[test]
    fn test_read_i16() {
        let reader = DataSource::new(&[0x01, 0x02]);
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_i16().unwrap(), 0x0201);
    }

    #[test]
    fn test_preloaded_reader_reads_full_content_with_random_access() {
        let data = b"Hello, world!";
        let source = DataSource::new(data.as_slice());
        let mut reader = source.preloaded_reader().unwrap();

        // Sequential read works.
        assert_eq!(reader.read_string(5).unwrap(), "Hello");
        assert_eq!(reader.position().unwrap(), 5);

        // Random access.
        reader.set_position(7).unwrap();
        assert_eq!(reader.read_string(5).unwrap(), "world");

        // Total length matches the source.
        let total = reader.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(total, data.len() as u64);
    }

    #[test]
    fn test_preloaded_reader_honors_offset_and_encoding() {
        // Source bytes: "Hello, " (skipped by offset) | <0xE9>llo
        // (window), where 0xE9 decodes to 'é' in WINDOWS-1252. The
        // preloaded reader should see only the 4-byte window
        // "<0xE9>llo" and decode it to "éllo".
        let bytes: &[u8] = &[
            0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x2C, 0x20, 0xE9, 0x6C, 0x6C, 0x6F,
        ];
        let source = DataSource::new_with_offset(bytes, 7, Some(4));
        let mut reader = source.preloaded_reader().unwrap();

        // Position starts at the window's beginning, not the
        // underlying source's offset.
        assert_eq!(reader.position().unwrap(), 0);

        // Length is the windowed length.
        let len = reader.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(len, 4);

        // Encoding is inherited — 0xE9 decodes to 'é'.
        reader.set_position(0).unwrap();
        assert_eq!(reader.read_string(4).unwrap(), "éllo");
    }

    #[test]
    fn test_read_to_string_utf8() {
        let reader = DataSource::new("Hello, world!".as_bytes());
        let mut reader = reader.reader().unwrap();
        let mut buf = String::new();
        let n = reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "Hello, world!");
        assert_eq!(n, "Hello, world!".len());
    }

    #[test]
    fn test_read_to_string_with_windows_1252_encoding() {
        // 0xE9 in WINDOWS-1252 is 'é' (U+00E9) - not valid UTF-8 on its own
        let bytes: &[u8] = &[0x48, 0xE9, 0x6C, 0x6C, 0x6F];
        let reader = DataSource::new(bytes);
        let mut reader = reader.reader().unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "Héllo");
    }

    #[test]
    fn test_read_to_string_appends_to_existing_buffer() {
        let reader = DataSource::new("world!".as_bytes());
        let mut reader = reader.reader().unwrap();
        let mut buf = String::from("Hello, ");
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, "Hello, world!");
    }

    #[test]
    fn test_read_to_string_with_invalid_utf8_returns_error() {
        use encoding_rs::UTF_8;
        let bytes: &[u8] = &[0xFF, 0xFE, 0xFD];
        let reader = DataSource::new(bytes).with_encoding(UTF_8);
        let mut reader = reader.reader().unwrap();
        let mut buf = String::from("untouched");
        let result = reader.read_to_string(&mut buf);
        assert!(result.is_err());
        assert_eq!(buf, "untouched");
    }

    #[test]
    fn test_read_to_string_respects_offset_and_limit() {
        let reader =
            DataSource::new_with_offset("Hello, world! Hello, world!".as_bytes(), 5, Some(7));
        let mut reader = reader.reader().unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, ", world");
    }

    #[test]
    fn test_bufread_read_line_utf8() {
        let reader = DataSource::new("Hello\nWorld\n".as_bytes());
        let mut reader = reader.reader().unwrap();

        let mut buf = String::new();
        let n = BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(buf, "Hello\n");
        assert_eq!(n, "Hello\n".len());

        buf.clear();
        BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(buf, "World\n");

        buf.clear();
        let n = BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(n, 0);
        assert_eq!(buf, "");
    }

    #[test]
    fn test_bufread_read_line_without_trailing_newline() {
        let reader = DataSource::new("only line".as_bytes());
        let mut reader = reader.reader().unwrap();
        let mut buf = String::new();
        BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(buf, "only line");
    }

    #[test]
    fn test_bufread_read_line_with_windows_1252_encoding() {
        // 0xE9 = 'é', 0xE0 = 'à' in WINDOWS-1252
        let bytes: &[u8] = &[0x48, 0xE9, 0x6C, b'\n', 0x57, 0xE0, 0x72];
        let reader = DataSource::new(bytes);
        let mut reader = reader.reader().unwrap();

        let mut buf = String::new();
        BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(buf, "Hél\n");

        buf.clear();
        BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(buf, "Wàr");
    }

    #[test]
    fn test_bufread_read_line_appends_to_existing_buffer() {
        let reader = DataSource::new("second\n".as_bytes());
        let mut reader = reader.reader().unwrap();
        let mut buf = String::from("first ");
        BufRead::read_line(&mut reader, &mut buf).unwrap();
        assert_eq!(buf, "first second\n");
    }

    #[test]
    fn test_bufread_read_line_with_invalid_utf8_returns_error() {
        use encoding_rs::UTF_8;
        let bytes: &[u8] = &[0xFF, 0xFE, b'\n', b'o', b'k'];
        let reader = DataSource::new(bytes).with_encoding(UTF_8);
        let mut reader = reader.reader().unwrap();
        let mut buf = String::from("untouched");
        let result = BufRead::read_line(&mut reader, &mut buf);
        assert!(result.is_err());
        assert_eq!(buf, "untouched");
    }

    #[test]
    fn test_read_take_uses_default_impl() {
        // The default Read::take wraps Self in std::io::Take and delegates
        // to our read impl, so the limit must be honored even though we
        // don't override take.
        let reader = DataSource::new("Hello, world!".as_bytes());
        let reader = reader.reader().unwrap();
        let mut limited = Read::take(reader, 5);
        let mut buf = Vec::new();
        limited.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"Hello");
    }

    #[test]
    fn test_data_generator() {
        use std::io::Write;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = Arc::new(AtomicUsize::new(0));

        let tmp_path = std::env::temp_dir().join(format!(
            "infinitier_test_data_generator_{}.tmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp_path);
        assert!(!tmp_path.exists());

        let tmp_path_clone = tmp_path.clone();
        let generator = TempFileGenerator::new(Box::new(move || {
            let count = call_count.fetch_add(1, Ordering::SeqCst);
            let mut file = std::fs::File::create(&tmp_path_clone)?;
            file.write_all(format!("Hello, World! - {}", count).as_bytes())?;
            Ok(tmp_path_clone.clone())
        }));

        let datasource = DataSource::new(Data::Generator(Arc::new(generator)));
        assert!(!tmp_path.exists());

        // First read should create the file.
        {
            assert_eq!(
                datasource.reader().unwrap().read_string(100).unwrap(),
                "Hello, World! - 0"
            );
            assert!(tmp_path.exists());
        }

        // Second read should not create the file again.
        {
            assert_eq!(
                datasource.reader().unwrap().read_string(100).unwrap(),
                "Hello, World! - 0"
            );
            assert!(tmp_path.exists());
        }

        // If the file is deleted, the generator should be called again.
        {
            std::fs::remove_file(&tmp_path).unwrap();
            assert!(!tmp_path.exists());

            assert_eq!(
                datasource.reader().unwrap().read_string(100).unwrap(),
                "Hello, World! - 1"
            );
            assert!(tmp_path.exists());
        }

        let _ = std::fs::remove_file(&tmp_path);
    }

    // ── DataSource::Concat ────────────────────────────────────────────

    fn ds(bytes: &'static [u8]) -> DataSource {
        DataSource::new(bytes)
    }

    #[test]
    fn test_concat_reads_each_part_in_order() {
        let concat = DataSource::new_concat(vec![ds(b"Hello, "), ds(b"world"), ds(b"!")]);
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "Hello, world!");
    }

    #[test]
    fn test_concat_len_sums_parts_without_reading() {
        let concat = DataSource::new_concat(vec![ds(b"abc"), ds(b"defg"), ds(b"hi")]);
        assert_eq!(concat.len().unwrap(), 9);
        assert!(!concat.is_empty().unwrap());
    }

    #[test]
    fn test_concat_read_spans_part_boundary() {
        // 5-byte read straddles the boundary between part 0 ("Hello, ")
        // and part 1 ("world"). The contract is: each underlying `read`
        // returns from only one part, but successive reads chain.
        let concat = DataSource::new_concat(vec![ds(b"Hello, "), ds(b"world!")]);
        let mut reader = concat.reader().unwrap();
        let mut buf = [0u8; 13];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"Hello, world!");
    }

    #[test]
    fn test_concat_seek_into_later_part() {
        // Seek lands inside the third part — only that part should be
        // opened (we can't easily observe that here, but we verify the
        // bytes are correct).
        let concat = DataSource::new_concat(vec![ds(b"AAAA"), ds(b"BBBB"), ds(b"CCCC")]);
        let mut reader = concat.reader().unwrap();
        reader.seek(std::io::SeekFrom::Start(9)).unwrap();
        let mut buf = [0u8; 3];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"CCC");
        assert_eq!(reader.stream_position().unwrap(), 12);
    }

    #[test]
    fn test_concat_seek_from_end_and_current() {
        let concat = DataSource::new_concat(vec![ds(b"abcd"), ds(b"efgh")]);
        let mut reader = concat.reader().unwrap();
        // From End: seek to 2 bytes before end (position 6).
        reader.seek(std::io::SeekFrom::End(-2)).unwrap();
        assert_eq!(reader.stream_position().unwrap(), 6);
        let mut tail = [0u8; 2];
        reader.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, b"gh");
        // From Current: rewind 4 bytes back to position 4.
        reader.seek(std::io::SeekFrom::Current(-4)).unwrap();
        assert_eq!(reader.stream_position().unwrap(), 4);
        let mut mid = [0u8; 2];
        reader.read_exact(&mut mid).unwrap();
        assert_eq!(&mid, b"ef");
    }

    #[test]
    fn test_concat_seek_past_end_errors() {
        let concat = DataSource::new_concat(vec![ds(b"abc")]);
        let mut reader = concat.reader().unwrap();
        // Seeking exactly to total_length is allowed (EOF marker)…
        reader.seek(std::io::SeekFrom::Start(3)).unwrap();
        // …but seeking past total_length is an error.
        let err = reader.seek(std::io::SeekFrom::Start(4)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_concat_empty_parts_skipped() {
        // Empty parts in the middle and ends must not stall the reader
        // (otherwise the read/fill_buf loops would spin forever).
        let concat = DataSource::new_concat(vec![ds(b""), ds(b"A"), ds(b""), ds(b"B"), ds(b"")]);
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "AB");
    }

    #[test]
    fn test_concat_of_zero_parts_is_empty() {
        let concat = DataSource::new_concat(vec![]);
        assert_eq!(concat.len().unwrap(), 0);
        assert!(concat.is_empty().unwrap());
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn test_concat_bufread_crosses_part_boundary() {
        use std::io::BufRead;
        // `read_until(b'\n')` should keep pulling from successive
        // parts until it finds the delimiter — the trickiest BufRead
        // case across boundaries.
        let concat = DataSource::new_concat(vec![ds(b"first "), ds(b"line\nleftover")]);
        let mut reader = concat.reader().unwrap();
        let mut buf = Vec::new();
        let n = reader.read_until(b'\n', &mut buf).unwrap();
        assert_eq!(buf, b"first line\n");
        assert_eq!(n, 11);
    }

    #[test]
    fn test_concat_nested() {
        // A Concat that contains another Concat as one of its parts.
        // The nested ConcatReader is created on demand.
        let inner = DataSource::new_concat(vec![ds(b"inner-"), ds(b"data")]);
        let outer = DataSource::new_concat(vec![ds(b"["), inner, ds(b"]")]);
        assert_eq!(outer.len().unwrap(), 1 + 10 + 1);
        let mut reader = outer.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "[inner-data]");
    }

    #[test]
    fn test_concat_path_parts_read_lazily() {
        // Confirm that Concat over file-backed parts (a) computes
        // length via stat (i.e. doesn't need to read content), and (b)
        // reads the bytes correctly through actual file I/O.
        use std::io::Write;
        let tmpdir = std::env::temp_dir();
        let pid = std::process::id();
        let p1 = tmpdir.join(format!("concat_test_a_{pid}.bin"));
        let p2 = tmpdir.join(format!("concat_test_b_{pid}.bin"));
        std::fs::File::create(&p1)
            .unwrap()
            .write_all(b"file-1-bytes")
            .unwrap();
        std::fs::File::create(&p2)
            .unwrap()
            .write_all(b"||file-2-bytes")
            .unwrap();

        let concat = DataSource::new_concat(vec![
            DataSource::new(p1.as_path()),
            DataSource::new(p2.as_path()),
        ]);
        // `len` does only stat calls — no content is read here.
        assert_eq!(concat.len().unwrap(), 12 + 14);

        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "file-1-bytes||file-2-bytes");

        let _ = std::fs::remove_file(&p1);
        let _ = std::fs::remove_file(&p2);
    }

    #[test]
    fn test_concat_does_not_call_generator_until_first_read() {
        // The temp-file generator counts how many times it's invoked.
        // Building a Concat (and calling `len`) must NOT count as a
        // "read" of the part's bytes — but `len` does need to realise
        // the file once to stat it. We assert that `len + reader()`
        // realises the file at most once, and that no extra
        // realisations happen for reads inside that part.
        use std::io::Write;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        // Process-id is enough to keep the file isolated from other
        // simultaneously-running test binaries — the test itself runs
        // single-threaded with respect to this path.
        let tmp_path =
            std::env::temp_dir().join(format!("concat_no_preload_{}.tmp", std::process::id()));
        let _ = std::fs::remove_file(&tmp_path);
        let tmp_clone = tmp_path.clone();
        let generator = TempFileGenerator::new(Box::new(move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            let mut f = std::fs::File::create(&tmp_clone)?;
            f.write_all(b"generated-payload")?;
            Ok(tmp_clone.clone())
        }));

        let gen_part = DataSource::new(Data::Generator(Arc::new(generator)));
        let concat = DataSource::new_concat(vec![ds(b"prefix-"), gen_part, ds(b"-suffix")]);

        // Length lookup realises the generator exactly once (to stat).
        assert_eq!(concat.len().unwrap(), 7 + 17 + 7);
        let after_len = call_count.load(Ordering::SeqCst);
        assert_eq!(after_len, 1, "len() should realise the generator once");

        // Reading every byte of the concatenation must NOT cause
        // additional realisations (the file is already on disk).
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "prefix-generated-payload-suffix");
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            after_len,
            "reading bytes should not re-realise the generator"
        );

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn test_concat_with_encoding() {
        use encoding_rs::UTF_8;
        let concat = DataSource::new_concat(vec![ds(b"hello"), ds(b"!")]).with_encoding(UTF_8);
        assert_eq!(concat.encoding().name(), "UTF-8");
        let mut reader = concat.reader().unwrap();
        assert_eq!(reader.read_string(6).unwrap(), "hello!");
    }

    #[test]
    fn test_concat_with_offset_skips_into_later_part() {
        // Offset 9 lands inside the third part ("ccc"); limit Some(2)
        // takes 2 bytes from there.
        let concat = DataSource::new_concat(vec![ds(b"aaaa"), ds(b"bbbb"), ds(b"cccc")])
            .with_offset(9, Some(2));
        assert_eq!(concat.len().unwrap(), 2);
        let mut reader = concat.reader().unwrap();
        // Take<T> normalises position to start at 0 — same convention
        // as `DataSource::Embedded` with offset+limit.
        assert_eq!(reader.position().unwrap(), 0);
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "cc");
    }

    #[test]
    fn test_concat_with_offset_spans_part_boundary() {
        // Offset 3 + limit 5 spans the join between part 1 (4 bytes)
        // and part 2 — the windowed reader should still hop between
        // parts transparently.
        let concat = DataSource::new_concat(vec![ds(b"aaaa"), ds(b"BBBB"), ds(b"cccc")])
            .with_offset(3, Some(5));
        assert_eq!(concat.len().unwrap(), 5);
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "aBBBB");
    }

    #[test]
    fn test_concat_with_offset_no_limit_skips_only() {
        // Offset without limit: window = sum_of_parts - offset.
        let concat = DataSource::new_concat(vec![ds(b"abcd"), ds(b"efgh")]).with_offset(5, None);
        assert_eq!(concat.len().unwrap(), 3);
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "fgh");
    }

    #[test]
    fn test_concat_with_offset_limit_caps_at_actual_length() {
        // limit larger than the bytes remaining after offset: visible
        // length should clamp to remaining_bytes, not panic / over-read.
        let concat = DataSource::new_concat(vec![ds(b"abc"), ds(b"def")]).with_offset(4, Some(100));
        assert_eq!(concat.len().unwrap(), 2);
        let mut reader = concat.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "ef");
    }

    #[test]
    fn test_concat_with_offset_seek_from_end_uses_window_length() {
        // SeekFrom::End must refer to the windowed end, not the
        // underlying concat's end — `Take<T: Seek>` enforces this.
        let concat = DataSource::new_concat(vec![ds(b"abcd"), ds(b"efgh")]).with_offset(2, Some(4));
        // Window contents = "cdef".
        let mut reader = concat.reader().unwrap();
        reader.seek(std::io::SeekFrom::End(-2)).unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "ef");
    }

    #[test]
    fn test_concat_with_offset_replaces_existing_window() {
        // Convention matches `Embedded`: chained `with_offset` calls
        // replace the previous window rather than composing.
        let a = DataSource::new_concat(vec![ds(b"aaaa"), ds(b"bbbb")]).with_offset(2, Some(4));
        // a's window is "aabb" (4 bytes).
        let b = a.with_offset(1, Some(2));
        // b re-bases against the underlying concat: offset=1, limit=2
        // → "aa" (positions 1..3 of "aaaabbbb").
        assert_eq!(b.len().unwrap(), 2);
        let mut reader = b.reader().unwrap();
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "aa");
    }

    #[test]
    fn test_concat_with_offset_zero_is_a_noop() {
        // offset=0 + limit=None must behave identically to a plain
        // `new_concat` (no Take wrapper, no seek).
        let plain = DataSource::new_concat(vec![ds(b"hello"), ds(b" world")]);
        let windowed =
            DataSource::new_concat(vec![ds(b"hello"), ds(b" world")]).with_offset(0, None);
        let mut a = String::new();
        let mut b = String::new();
        plain.reader().unwrap().read_to_string(&mut a).unwrap();
        windowed.reader().unwrap().read_to_string(&mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "hello world");
    }

    #[test]
    fn test_concat_reader_supports_position_round_trip() {
        // Position-tracking on the ConcatReader must agree with
        // `stream_position` after each operation.
        let concat = DataSource::new_concat(vec![ds(b"AAAA"), ds(b"BBBB"), ds(b"CC")]);
        let mut reader = concat.reader().unwrap();
        assert_eq!(reader.position().unwrap(), 0);
        let mut buf = [0u8; 6];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"AAAABB");
        assert_eq!(reader.position().unwrap(), 6);
        reader.seek(std::io::SeekFrom::Start(2)).unwrap();
        assert_eq!(reader.position().unwrap(), 2);
        let mut buf2 = [0u8; 4];
        reader.read_exact(&mut buf2).unwrap();
        assert_eq!(&buf2, b"AABB");
    }
}
