//! BIF V1.0 envelope: a single zlib stream wrapping an entire BIFF V1
//! archive, prefixed by a small header describing the inner archive.
//!
//! Wire layout:
//!
//! ```text
//! 0       "BIF V1.0"            (8 bytes)
//! 8       name_length           (u32, little-endian; includes null terminator)
//! 12      name                  (`name_length` bytes, null-terminated ASCII)
//! 12+nl   uncompressed_size     (u32)
//! 16+nl   compressed_size       (u32)
//! 20+nl   <zlib-compressed BIFF V1 payload>
//! ```
//!
//! The importer reads but discards the embedded `name` field, so the exact
//! string written here is not load-bearing for round-tripping.

use std::io::{self, Write};

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::BIF_V1_0_SIGNATURE;

pub(super) fn write_bif_v1<W: Write>(biff_bytes: &[u8], name: &str, writer: &mut W) -> io::Result<()> {
    // Compress the BIFF V1 payload into a buffer first so we can write the
    // header (which contains both uncompressed and compressed sizes) in one
    // go. The archives we round-trip are small enough that an extra
    // in-memory copy is cheaper than streaming with two seeks.
    let mut compressed: Vec<u8> = Vec::with_capacity(biff_bytes.len() / 2);
    {
        let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
        encoder.write_all(biff_bytes)?;
        encoder.finish()?;
    }

    let name_bytes = name.as_bytes();
    let name_length = u32::try_from(name_bytes.len() + 1)
        .map_err(|_| io::Error::other("BIF V1.0 name too long"))?;
    let uncompressed_size =
        u32::try_from(biff_bytes.len()).map_err(|_| io::Error::other("BIF V1.0 inner payload exceeds 4 GiB"))?;
    let compressed_size =
        u32::try_from(compressed.len()).map_err(|_| io::Error::other("BIF V1.0 zlib stream exceeds 4 GiB"))?;

    writer.write_all(BIF_V1_0_SIGNATURE.as_bytes())?;
    writer.write_all(&name_length.to_le_bytes())?;
    writer.write_all(name_bytes)?;
    writer.write_all(&[0])?; // null terminator counted in name_length
    writer.write_all(&uncompressed_size.to_le_bytes())?;
    writer.write_all(&compressed_size.to_le_bytes())?;
    writer.write_all(&compressed)?;
    Ok(())
}
