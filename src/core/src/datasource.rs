use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek},
    path::{Path, PathBuf},
    sync::Arc,
};

use encoding_rs::{Encoding, WINDOWS_1252};
use flate2::bufread::ZlibDecoder;

/// A data importer.
/// Parses data from a data source and returns the parsed data
pub trait Importer {
    type T;
    /// Imports a data source
    fn import(source: &DataSource) -> std::io::Result<Self::T>;
}

/// A data source
#[derive(Debug, Clone)]
pub enum Data {
    FileSource(PathBuf),
    MemorySource(Arc<Vec<u8>>),
}

impl From<PathBuf> for Data {
    fn from(value: PathBuf) -> Self {
        Data::FileSource(value)
    }
}

impl From<&Path> for Data {
    fn from(value: &Path) -> Self {
        Data::FileSource(value.to_path_buf())
    }
}

impl From<&str> for Data {
    fn from(value: &str) -> Self {
        Data::FileSource(PathBuf::from(value))
    }
}

impl From<Vec<u8>> for Data {
    fn from(value: Vec<u8>) -> Self {
        Data::MemorySource(Arc::new(value))
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

pub trait DataTrait: BufRead + Seek {}

impl DataTrait for BufReader<File> {}
impl DataTrait for Cursor<&[u8]> {}

impl Data {
    /// Returns a reader for the data
    pub fn data(&self) -> std::io::Result<Box<dyn DataTrait + '_>> {
        match self {
            Data::FileSource(reader) => Ok(Box::new(BufReader::new(File::open(reader)?))),
            Data::MemorySource(reader) => Ok(Box::new(Cursor::new(reader.as_slice()))),
        }
    }
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
    pub fn new_with_offset<D: Into<Data>>(data: D, offset: u64) -> Self {
        DataSource::Embedded {
            encoding: WINDOWS_1252,
            data: data.into(),
            offset,
        }
    }

    /// Sets the encoding
    pub fn with_encoding(self, encoding: &'static Encoding) -> Self {
        match self {
            DataSource::Full { data, .. } => DataSource::Full { encoding, data },
            DataSource::Embedded { data, offset, .. } => DataSource::Embedded {
                encoding,
                data,
                offset,
            },
        }
    }

    /// Sets the offset
    pub fn with_offset(self, offset: u64) -> Self {
        match self {
            DataSource::Full { encoding, data } => DataSource::Embedded {
                encoding,
                data,
                offset,
            },
            DataSource::Embedded { encoding, data, .. } => DataSource::Embedded {
                encoding,
                data,
                offset,
            },
        }
    }

    /// Returns the encoding
    pub fn encoding(&self) -> &'static Encoding {
        match self {
            DataSource::Full { encoding, .. } => encoding,
            DataSource::Embedded { encoding, .. } => encoding,
        }
    }

    /// Creates a data reader
    pub fn reader(&self) -> std::io::Result<Reader<'_>> {
        match self {
            DataSource::Full { encoding, data } => Ok(Reader {
                data: data.data()?,
                charset: encoding,
            }),
            DataSource::Embedded {
                encoding,
                data,
                offset,
            } => {
                let mut data = data.data()?;
                data.seek(std::io::SeekFrom::Start(*offset))?;
                Ok(Reader {
                    data,
                    charset: encoding,
                })
            }
        }
    }
}

/// A reader that reads a byte array with a specific encoding
pub struct Reader<'a> {
    data: Box<dyn DataTrait + 'a>,
    charset: &'static Encoding,
}

impl<'a> Reader<'a> {
    /// Returns a zip reader
    pub fn as_zip_reader(&mut self) -> ZipReader<'_> {
        ZipReader {
            data: ZlibDecoder::new(self.data.as_mut()),
            charset: self.charset,
        }
    }

    /// Reads a line from the current position
    /// and returns it as a `String` and the number of bytes read.
    /// If bytes read is 0, then EOF has been reached
    pub fn read_line(&mut self) -> std::io::Result<(String, usize)> {
        let mut buf = String::new();
        let bytes = self.data.read_line(&mut buf)?;
        Ok((buf, bytes))
    }

    /// Read the first `n_chars` characters from a byte array interpreted
    /// with the Reader `charset`, and return them as a `String`.
    pub fn read_string(&mut self, size: u64) -> std::io::Result<String> {
        let mut buf = vec![];
        let mut chunk = (&mut self.data).take(size);
        // Do appropriate error handling for your situation
        // Maybe it's OK if you didn't read enough bytes?
        chunk.read_to_end(&mut buf)?;
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

    /// Reads exactly `N` bytes from the current position and returns them as a byte array.
    ///
    /// If the end of the file is reached before `N` bytes could be read, an `io::Error` is returned.
    pub fn read_exact<const N: usize>(&mut self) -> std::io::Result<[u8; N]> {
        let mut buf = [0u8; N];
        self.data.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads up to `N` bytes from the current position and returns them as a tuple of a byte array and the number of bytes read.
    pub fn read_at_most<const N: usize>(&mut self) -> std::io::Result<([u8; N], usize)> {
        let mut buf = [0u8; N];
        let n = self.data.read(&mut buf)?;
        Ok((buf, n))
    }

    /// Reads up to `N` bytes from the current position and returns them as a `Vec<u8>`.
    ///
    /// If the end of the file is reached before `N` bytes could be read, the returned
    /// `Vec<u8>` will contain less than `N` elements.
    pub fn read_at_most_to_vec<const N: usize>(&mut self) -> std::io::Result<Vec<u8>> {
        let (buf, n) = self.read_at_most::<N>()?;
        Ok(buf[..n].to_vec())
    }

    /// Reads a i32 from the current position
    pub fn read_i32(&mut self) -> std::io::Result<i32> {
        Ok(i32::from_le_bytes(self.read_exact::<4>()?))
    }

    /// Reads a u32 from the current position
    pub fn read_u32(&mut self) -> std::io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact::<4>()?))
    }

    /// Reads a u16 from the current position
    pub fn read_u16(&mut self) -> std::io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact::<2>()?))
    }

    /// Returns the current position of the cursor
    pub fn position(&mut self) -> std::io::Result<u64> {
        self.data.stream_position()
    }

    /// Sets the position of the cursor
    pub fn set_position(&mut self, pos: u64) -> std::io::Result<u64> {
        self.data.seek(std::io::SeekFrom::Start(pos))
    }

    /// Reads a line from the offset position
    /// and returns it as a `String` and the number of bytes read.
    /// If bytes read is 0, then EOF has been reached
    pub fn read_line_at(&mut self, offset: u64) -> std::io::Result<(String, usize)> {
        self.data.seek(std::io::SeekFrom::Start(offset))?;
        self.read_line()
    }

    /// Reads a string from the offset position
    pub fn read_string_at(&mut self, offset: u64, size: u64) -> std::io::Result<String> {
        self.data.seek(std::io::SeekFrom::Start(offset))?;
        self.read_string(size)
    }

    /// Reads a u32 from the offset position
    pub fn read_u32_at(&mut self, offset: u64) -> std::io::Result<u32> {
        self.data.seek(std::io::SeekFrom::Start(offset))?;
        self.read_u32()
    }

    /// Reads a i32 from the offset position
    pub fn read_i32_at(&mut self, offset: u64) -> std::io::Result<i32> {
        self.data.seek(std::io::SeekFrom::Start(offset))?;
        self.read_i32()
    }

    /// Reads a u16 from the offset position
    pub fn read_u16_at(&mut self, offset: u64) -> std::io::Result<u16> {
        self.data.seek(std::io::SeekFrom::Start(offset))?;
        self.read_u16()
    }
}

/// A reader that reads a byte array with a specific encoding from a zip encoded source
pub struct ZipReader<'a> {
    data: ZlibDecoder<&'a mut dyn DataTrait>,
    charset: &'static Encoding,
}

impl<'a> ZipReader<'a> {
    /// Skips `size` bytes
    pub fn skip(&mut self, size: u64) -> std::io::Result<()> {
        for _ in 0..size {
            self.read_u8()?;
        }
        Ok(())
    }

    /// Read the first `n_chars` characters from a byte array interpreted
    /// with the Reader `charset`, and return them as a `String`.
    pub fn read_string(&mut self, size: u64) -> std::io::Result<String> {
        let mut buf = vec![];
        let mut chunk = (&mut self.data).take(size);
        // Do appropriate error handling for your situation
        // Maybe it's OK if you didn't read enough bytes?
        chunk.read_to_end(&mut buf)?;
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

    /// Reads exactly `N` bytes from the current position and returns them as a byte array.
    ///
    /// If the end of the file is reached before `N` bytes could be read, an `io::Error` is returned.
    pub fn read_exact<const N: usize>(&mut self) -> std::io::Result<[u8; N]> {
        let mut buf = [0u8; N];
        self.data.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads up to `N` bytes from the current position and returns them as a tuple of a byte array and the number of bytes read.
    pub fn read_at_most<const N: usize>(&mut self) -> std::io::Result<([u8; N], usize)> {
        let mut buf = [0u8; N];
        let n = self.data.read(&mut buf)?;
        Ok((buf, n))
    }

    /// Reads up to `N` bytes from the current position and returns them as a `Vec<u8>`.
    ///
    /// If the end of the file is reached before `N` bytes could be read, the returned
    /// `Vec<u8>` will contain less than `N` elements.
    pub fn read_at_most_to_vec<const N: usize>(&mut self) -> std::io::Result<Vec<u8>> {
        let (buf, n) = self.read_at_most::<N>()?;
        Ok(buf[..n].to_vec())
    }

    /// Reads a i32 from the current position
    pub fn read_i32(&mut self) -> std::io::Result<i32> {
        Ok(i32::from_le_bytes(self.read_exact::<4>()?))
    }

    /// Reads a u32 from the current position
    pub fn read_u32(&mut self) -> std::io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact::<4>()?))
    }

    /// Reads a u16 from the current position
    pub fn read_u16(&mut self) -> std::io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact::<2>()?))
    }

    /// Reads a u8 from the current position
    #[inline]
    pub fn read_u8(&mut self) -> std::io::Result<u8> {
        Ok(u8::from_le_bytes(self.read_exact::<1>()?))
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_read_string() {
        let reader = DataSource::new("Hello, world!".as_bytes());
        let mut reader = reader.reader().unwrap();
        assert_eq!(reader.read_string(5).unwrap(), "Hello");
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
}
