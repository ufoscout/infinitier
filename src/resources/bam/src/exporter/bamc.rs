//! BAMC writer.
//!
//! Wire layout:
//!
//! ```text
//! 0   "BAMCV1  "         (8 bytes)
//! 8   uncompressed_size  u32 (= bam_v1_bytes.len())
//! 12  <zlib-compressed BAM V1 payload>
//! ```

use std::io::{self, Write};

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::BAMC_SIGNATURE;

pub(super) fn write_bamc<W: Write>(bam_v1_bytes: &[u8], writer: &mut W) -> io::Result<()> {
    let uncompressed_size = u32::try_from(bam_v1_bytes.len())
        .map_err(|_| io::Error::other("BAM V1 payload exceeds 4 GiB"))?;

    writer.write_all(BAMC_SIGNATURE.as_bytes())?;
    writer.write_all(&uncompressed_size.to_le_bytes())?;

    let mut encoder = ZlibEncoder::new(writer, Compression::default());
    encoder.write_all(bam_v1_bytes)?;
    encoder.finish()?;
    Ok(())
}
