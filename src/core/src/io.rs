use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek},
    path::Path,
};

use encoding_rs::Encoding;

/// A trait for importing data
pub trait Importer {
    type T;
    fn import(&self) -> std::io::Result<Self::T>;
}

/// A reader that reads a byte array with a specific encoding
pub struct Reader<B: BufRead> {
    data: B,
    charset: &'static Encoding,
}

impl Reader<BufReader<File>> {
    /// Reads a file with a specific encoding
    pub fn with_file(
        path: &Path,
        charset: &'static Encoding,
    ) -> Result<Reader<BufReader<File>>, std::io::Error> {
        Ok(Reader {
            data: BufReader::new(File::open(path)?),
            charset,
        })
    }
}

impl<B: BufRead> Reader<B> {
    /// Creates a new Reader
    pub fn new(data: B, charset: &'static Encoding) -> Reader<B> {
        Reader { data, charset }
    }

    /// Reads a line from the current position
    /// and returns it as a `String` and the number of bytes read.
    /// If bytes read is 0, then EOF has been reached
    pub fn read_line(&mut self) -> std::io::Result<(String, usize)> {
        let mut buf = String::new();
        let bytes  = self.data.read_line(&mut buf)?;
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
}

impl<B: BufRead + Seek> Reader<B> {
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use encoding_rs::WINDOWS_1252;

    use super::*;

    #[test]
    fn test_read_string() {
        let mut reader = Reader::new("Hello, world!".as_bytes(), WINDOWS_1252);
        assert_eq!(reader.read_string(5).unwrap(), "Hello");
    }

    #[test]
    fn test_read_string_at() {
        let mut reader = Reader::new(Cursor::new("Hello, world!".as_bytes()), WINDOWS_1252);
        assert_eq!(reader.read_string_at(7, 5).unwrap(), "world");
    }

    #[test]
    fn test_read_u32() {
        let mut reader = Reader::new(Cursor::new(&[0x01, 0x02, 0x03, 0x04]), WINDOWS_1252);
        assert_eq!(reader.read_u32().unwrap(), 0x04030201);
    }

    #[test]
    fn test_read_u32_at() {
        let mut reader = Reader::new(
            Cursor::new(&[0x01, 0x02, 0x01, 0x01, 0x03, 0x04]),
            WINDOWS_1252,
        );
        assert_eq!(reader.read_u32_at(2).unwrap(), 0x04030101);
    }

    #[test]
    fn test_read_i32() {
        let mut reader = Reader::new(Cursor::new(&[0x01, 0x02, 0x03, 0x04]), WINDOWS_1252);
        assert_eq!(reader.read_i32().unwrap(), 0x04030201);
    }

    #[test]
    fn test_read_i32_at() {
        let mut reader = Reader::new(
            Cursor::new(&[0x01, 0x01, 0x01, 0x02, 0x01, 0x04]),
            WINDOWS_1252,
        );
        assert_eq!(reader.read_i32_at(2).unwrap(), 0x04010201);
    }

    #[test]
    fn test_read_u16() {
        let mut reader = Reader::new(Cursor::new(&[0x01, 0x02]), WINDOWS_1252);
        assert_eq!(reader.read_u16().unwrap(), 0x0201);
    }

    #[test]
    fn test_read_u16_at() {
        let mut reader = Reader::new(Cursor::new(&[0x01, 0x02, 0x03, 0x04]), WINDOWS_1252);
        assert_eq!(reader.read_u16_at(2).unwrap(), 0x0403);
    }
}
