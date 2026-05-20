//! MOSC writer.
//!
//! Wire layout:
//!
//! ```text
//! 0   "MOSCV1  "         (8 bytes)
//! 8   uncompressed_size  u32 (= mos_v1_bytes.len())
//! 12  <zlib-compressed MOS V1 payload>
//! ```

use std::io::{self, Write};

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::MOSC_V1_SIGNATURE;

pub(super) fn write_mosc<W: Write>(mos_v1_bytes: &[u8], writer: &mut W) -> io::Result<()> {
    let uncompressed_size = u32::try_from(mos_v1_bytes.len())
        .map_err(|_| io::Error::other("MOS V1 payload exceeds 4 GiB"))?;

    writer.write_all(MOSC_V1_SIGNATURE)?;
    writer.write_all(&uncompressed_size.to_le_bytes())?;

    let mut encoder = ZlibEncoder::new(writer, Compression::default());
    encoder.write_all(mos_v1_bytes)?;
    encoder.finish()?;
    Ok(())
}
