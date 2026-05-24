//! SAV V1 reader.

use std::io::Read;

use infinitier_common::ResourceType;
use infinitier_datasource::{DataSource, Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{SAV_V1_SIGNATURE, Sav, SavEntry};

/// A SAV save-game archive importer.
pub struct SavImporter<'a> {
    pub name: &'a str,
}

impl Importer for SavImporter<'_> {
    type T = Sav;

    fn import(&self, source: &DataSource) -> std::io::Result<Sav> {
        let mut reader = source.reader()?;
        let mut signature = [0u8; 8];
        reader.read_exact(&mut signature)?;
        if &signature != SAV_V1_SIGNATURE {
            error!("Unsupported SAV signature in {}: {signature:?}", self.name);
            return Err(std::io::Error::other(format!(
                "Unsupported SAV signature: {signature:?}",
            )));
        }

        let mut entries: Vec<SavEntry> = Vec::new();
        loop {
            // Peek: if we're at EOF the entry loop is done. We can't
            // just rely on `read_u32()`'s EOF error because that's
            // shape-compatible with a truncated file, which we *do*
            // want to surface — so we explicitly check.
            let pos = reader.position()?;
            let mut peek = [0u8; 1];
            match reader.read(&mut peek)? {
                0 => break, // clean EOF between entries
                _ => reader.set_position(pos)?,
            };

            let entry = read_entry(&mut reader)?;
            entries.push(entry);
        }

        debug!("Loaded {} [SAV V1]: {} entries", self.name, entries.len());
        Ok(Sav { entries })
    }
}

/// Reads one [`SavEntry`] starting at the reader's current position,
/// inflating the zlib payload inline.
fn read_entry<R: std::io::BufRead + std::io::Seek>(
    reader: &mut Reader<R>,
) -> std::io::Result<SavEntry> {
    let filename_length = reader.read_u32()? as usize;
    if filename_length == 0 {
        return Err(std::io::Error::other(
            "SAV entry has filename_length=0; format requires at least the NUL terminator",
        ));
    }
    // `filename_length` is inclusive of the trailing NUL byte — read
    // `length` bytes and strip it. Trailing garbage is also stripped
    // (some saves pad with NULs past the actual name end).
    let mut filename_buf = vec![0u8; filename_length];
    reader.read_exact(&mut filename_buf)?;
    let trimmed_end = filename_buf
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(filename_buf.len());
    let filename = std::str::from_utf8(&filename_buf[..trimmed_end])
        .map_err(|e| std::io::Error::other(format!("filename is not UTF-8: {e}")))?
        .to_owned();

    let uncompressed_size = reader.read_u32()?;
    // The on-disk `compressed_size` is informational — the zlib stream
    // self-terminates at its adler32 trailer, which is exactly where
    // `as_zip_reader()` stops consuming bytes from the outer reader.
    // We read past the field so the cursor advances, but don't rely
    // on its value (same approach MOSC's importer takes).
    let _compressed_size = reader.read_u32()?;

    // Inflate straight out of the outer reader — `as_zip_reader` is
    // the shared codebase idiom for "wrap the next bytes as a zlib
    // stream", and once it returns EOF the outer reader is left
    // positioned right after the zlib trailer, ready for the next
    // entry record.
    let mut data = Vec::with_capacity(uncompressed_size as usize);
    reader.as_zip_reader().read_to_end(&mut data).map_err(|e| {
        std::io::Error::other(format!("SAV entry '{filename}': zlib inflate failed: {e}"))
    })?;
    if data.len() as u64 != uncompressed_size as u64 {
        return Err(std::io::Error::other(format!(
            "SAV entry '{filename}': declared uncompressed_size={uncompressed_size}, \
             actual={}",
            data.len()
        )));
    }

    // Resolve the resource type from the filename's extension.
    let r#type = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .and_then(ResourceType::from_extension)
        .unwrap_or(ResourceType::Unknown(0));

    Ok(SavEntry {
        filename,
        r#type,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn test_parse_baldur_sav_index() {
        let sav = baldur_sav();
        assert!(!sav.entries.is_empty(), "SAV index must be non-empty");

        let first = &sav.entries[0];
        assert_eq!(first.filename, "AR0100.ARE");
        assert_eq!(first.r#type, ResourceType::Are);
        assert_eq!(first.data.len(), 75915);
        // Areas start with the "AREAV1.0" / "AREAV9.1" signature —
        // proves the data really is inflated (compressed bytes start
        // with the zlib `0x78 0x01` magic).
        assert_eq!(&first.data[..4], b"AREA");
    }

    #[test]
    fn test_every_entry_inflated_during_import() {
        // The eager inflate runs in `read_entry`; if any entry's zlib
        // payload had failed to inflate or had a size mismatch,
        // `baldur_sav()` itself would have panicked. Walking the
        // corpus here just confirms every entry came back with a
        // non-empty `data` buffer.
        let sav = baldur_sav();
        for entry in &sav.entries {
            assert!(
                !entry.data.is_empty(),
                "entry '{}' has empty data after import",
                entry.filename,
            );
        }
    }

    #[test]
    fn test_entry_type_matches_extension() {
        // BG's Baldur.sav contains 141 .ARE + 20 .STO entries — every
        // entry's resolved type must agree with its filename's
        // extension. Catches off-by-one in the extension split as
        // well as any regression in `ResourceType::from_extension`.
        let sav = baldur_sav();
        let mut are = 0;
        let mut sto = 0;
        for entry in &sav.entries {
            let ext = entry
                .filename
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_uppercase())
                .unwrap_or_default();
            match (ext.as_str(), entry.r#type) {
                ("ARE", ResourceType::Are) => are += 1,
                ("STO", ResourceType::Sto) => sto += 1,
                (e, t) => panic!("entry '{}' has ext={e} but type={t:?}", entry.filename),
            }
        }
        assert_eq!((are, sto), (141, 20));
    }

    #[test]
    fn test_unknown_extension_falls_back_to_unknown_zero() {
        // Synthetic minimal SAV with one entry whose extension isn't a
        // known IE resource type — type must be Unknown(0), not a
        // panic or some accidentally-matching variant. Also confirms
        // the inflated bytes round-trip through import.
        let sav = SavImporter { name: "synthetic" }
            .import(&DataSource::new(build_synthetic_sav("test.xyz", b"hello")))
            .unwrap();
        assert_eq!(sav.entries.len(), 1);
        assert_eq!(sav.entries[0].filename, "test.xyz");
        assert_eq!(sav.entries[0].r#type, ResourceType::Unknown(0));
        assert_eq!(sav.entries[0].data, b"hello");
    }

    #[test]
    fn test_rejects_corrupt_zlib_stream() {
        // Inflate now happens during import — a corrupt zlib payload
        // surfaces as an import error rather than being deferred to a
        // later `data_source` call. The error message names the
        // failing entry so operators can find it.
        let mut bytes = build_synthetic_sav("bad.are", b"hello");
        // Flip the zlib stream's last byte (the adler32 trailer) to
        // break decompression.
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let err = SavImporter { name: "corrupt" }
            .import(&DataSource::new(bytes))
            .unwrap_err();
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("zlib") || msg.contains("inflate") || msg.contains("deflate"),
            "expected a zlib/inflate error, got: {err}"
        );
        assert!(
            msg.contains("bad.are"),
            "error should mention the failing entry's name, got: {err}"
        );
    }

    #[test]
    fn test_rejects_wrong_signature() {
        let data = DataSource::new(&b"GARBAGE!"[..]);
        let err = SavImporter { name: "junk" }.import(&data).unwrap_err();
        assert!(err.to_string().contains("Unsupported SAV signature"));
    }

    #[test]
    fn test_rejects_empty_file() {
        let data = DataSource::new(&b""[..]);
        let err = SavImporter { name: "empty" }.import(&data).unwrap_err();
        // EOF before the signature is an error of some kind; we just
        // care that we don't silently produce an empty Sav.
        assert!(
            err.kind() == std::io::ErrorKind::UnexpectedEof
                || err.to_string().contains("signature")
        );
    }
}
