//! BIFC V1.0 envelope: the underlying BIFF V1 archive is split into
//! fixed-size chunks (32 KiB by default — see [`BIFC_CHUNK_SIZE`]), each
//! compressed with its own zlib stream and prefixed by its uncompressed
//! and compressed sizes.
//!
//! Wire layout:
//!
//! ```text
//! 0   "BIFCV1.0"               (8 bytes)
//! 8   total_uncompressed_size  (u32 — sum of inner BIFF V1 length)
//! 12  <chunk> <chunk> ...      (one chunk per CHUNK_SIZE-byte slice)
//! ```
//!
//! Per-chunk layout:
//!
//! ```text
//! 0   uncompressed_size  (u32)
//! 4   compressed_size    (u32)
//! 8   <zlib-compressed chunk bytes>
//! ```
//!
//! The importer ([`BifcCompressedReader`]) decompresses chunks lazily and
//! tolerates any chunk size, so the 32 KiB choice is internal.
//!
//! [`BifcCompressedReader`]: crate::importer::bifc

use std::io::{self, Write};

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::BIFCV1_0_SIGNATURE;

use super::BIFC_CHUNK_SIZE;

pub(super) fn write_bifc_v1_0<W: Write>(biff_bytes: &[u8], writer: &mut W) -> io::Result<()> {
    let total_uncompressed = u32::try_from(biff_bytes.len())
        .map_err(|_| io::Error::other("BIFC V1.0 inner payload exceeds 4 GiB"))?;

    writer.write_all(BIFCV1_0_SIGNATURE.as_bytes())?;
    writer.write_all(&total_uncompressed.to_le_bytes())?;

    for chunk in biff_bytes.chunks(BIFC_CHUNK_SIZE as usize) {
        let mut compressed: Vec<u8> = Vec::with_capacity(chunk.len() / 2);
        {
            let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
            encoder.write_all(chunk)?;
            encoder.finish()?;
        }
        let uncompressed_size = chunk.len() as u32;
        let compressed_size = u32::try_from(compressed.len())
            .map_err(|_| io::Error::other("BIFC V1.0 chunk zlib stream exceeds 4 GiB"))?;
        writer.write_all(&uncompressed_size.to_le_bytes())?;
        writer.write_all(&compressed_size.to_le_bytes())?;
        writer.write_all(&compressed)?;
    }
    Ok(())
}
