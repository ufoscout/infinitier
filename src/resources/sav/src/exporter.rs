//! SAV V1 writer.
//!
//! Serialises a [`Sav`] back to the on-disk format, re-compressing
//! every entry's [`SavEntry::data`] with zlib on the way out.

use std::io::{self, BufWriter, Write};
use std::path::Path;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::{SAV_V1_SIGNATURE, Sav, SavEntry};

/// A SAV save-game archive exporter.
pub struct SavExporter;

impl SavExporter {
    /// Writes `sav` as a SAV V1 archive into `writer`. Each entry's
    /// `data` is zlib-compressed (level
    /// [`Compression::default`]) before being emitted.
    pub fn export<W: Write>(&self, sav: &Sav, writer: &mut W) -> io::Result<()> {
        writer.write_all(SAV_V1_SIGNATURE)?;
        for entry in &sav.entries {
            write_entry(writer, entry)?;
        }
        Ok(())
    }

    /// Writes `sav` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, sav: &Sav, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(sav, &mut writer)?;
        writer.flush()
    }
}

/// Emit one entry record: length-prefixed filename (NUL-inclusive),
/// uncompressed_size (= `entry.data.len()`), compressed_size, then
/// the freshly-deflated zlib payload.
fn write_entry<W: Write>(writer: &mut W, entry: &SavEntry) -> io::Result<()> {
    let filename_bytes = entry.filename.as_bytes();
    // +1 for the trailing NUL — the SAV format declares the length
    // inclusive of it. See `crate::importer::read_entry`.
    let length_with_nul = filename_bytes.len() as u32 + 1;

    // Deflate first so we can write `compressed_size` correctly into
    // the entry header — the format requires it ahead of the
    // compressed payload, which means we can't stream.
    let mut compressed = Vec::new();
    {
        let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
        encoder.write_all(&entry.data)?;
        encoder.finish()?;
    }

    writer.write_all(&length_with_nul.to_le_bytes())?;
    writer.write_all(filename_bytes)?;
    writer.write_all(&[0u8])?;
    writer.write_all(&(entry.data.len() as u32).to_le_bytes())?;
    writer.write_all(&(compressed.len() as u32).to_le_bytes())?;
    writer.write_all(&compressed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use super::*;
    use crate::SavImporter;
    use crate::test_support::build_synthetic_sav;

    /// Recursive walk: collects every `.sav` file under `root`.
    /// `infinitier_test_utils` only ships a single-directory helper,
    /// and the SAV fixture tree is two levels deep, so we inline a
    /// small walker.
    fn find_savs_recursive(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.eq_ignore_ascii_case("sav"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        visit(root, &mut out);
        out
    }

    #[test]
    fn test_corpus_round_trip() {
        // For every `.sav` under `assets/SAV_GAM/`: import → export →
        // re-import → equal `Sav` struct. The intermediate bytes are
        // *not* expected to match the source — zlib re-compression
        // produces an equivalent-but-different stream — so this test
        // checks **structural** equality only. See the module docs.
        let root = get_assets_path().join("SAV_GAM");
        let paths = find_savs_recursive(&root);
        assert!(
            !paths.is_empty(),
            "no SAV fixtures under {}",
            root.display()
        );

        for path in paths {
            let original = SavImporter { name: "sav_rt" }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {}: {e}", path.display()));

            let mut produced: Vec<u8> = Vec::new();
            SavExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {}: {e}", path.display()));

            let re_imported = SavImporter { name: "sav_rt2" }
                .import(&DataSource::new(produced))
                .unwrap_or_else(|e| panic!("re-import {}: {e}", path.display()));
            assert_eq!(
                re_imported,
                original,
                "Sav struct mismatch for {}",
                path.display(),
            );
        }
    }

    #[test]
    fn test_export_to_file_round_trip() {
        // File-path overload — same struct-equality round-trip, but
        // exercises the `BufWriter<File>` path.
        let path = get_assets_path()
            .join("SAV_GAM/bg/Save/888888888-Mission-Pack-Save/Baldur.sav");
        let original = SavImporter { name: "rt_file" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        SavExporter
            .export_to_file(&original, tmp.path())
            .unwrap();
        let re_imported = SavImporter { name: "rt_file2" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        assert_eq!(re_imported, original);
    }

    #[test]
    fn test_export_synthetic_minimal_sav() {
        // Sanity check the lowest level: synthesise a single-entry
        // SAV with a known plaintext, parse it, re-export, and
        // confirm the inflated content survives.
        let payload = b"hello world";
        let source = build_synthetic_sav("test.are", payload);
        let original = SavImporter { name: "synth" }
            .import(&DataSource::new(source))
            .unwrap();
        assert_eq!(original.entries[0].data, payload);

        let mut produced = Vec::new();
        SavExporter.export(&original, &mut produced).unwrap();

        let re_imported = SavImporter { name: "synth2" }
            .import(&DataSource::new(produced))
            .unwrap();
        assert_eq!(re_imported, original);
        assert_eq!(re_imported.entries[0].data, payload);
    }
}
