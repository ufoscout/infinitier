use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::Ids;

/// An IDS file exporter.
///
/// Writes an [`Ids`] back to the textual IDS format. The output is
/// functionally — not byte-exactly — equivalent to the source: column
/// alignment and the header line may differ, but re-importing the emitted
/// text yields an equal [`Ids`].
///
/// Each entry is written as `<value_str> <name>` so that the original
/// numeric literal (decimal, hex, octal) is preserved and round-trips
/// through `IdsImporter`.
pub struct IdsExporter;

impl IdsExporter {
    /// Writes `ids` as IDS text into `writer`.
    pub fn export<W: Write>(&self, ids: &Ids, writer: &mut W) -> io::Result<()> {
        // The first line is a recognised IDS header — `IdsImporter` skips
        // any line whose first whitespace-separated token does not parse as
        // an integer, so "IDS V1.0" is treated as a no-op marker. Vanilla
        // BG/IWD ship a mix of headers (the literal "IDS V1.0", a bare
        // entry count, or nothing at all); we emit the most common form.
        writeln!(writer, "IDS V1.0")?;
        for entry in &ids.entries {
            writeln!(writer, "{} {}", entry.value_str, entry.name)?;
        }
        Ok(())
    }

    /// Writes `ids` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, ids: &Ids, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(ids, &mut writer)?;
        writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdsEntry, IdsImporter};
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path};

    fn roundtrip(ids: &Ids) -> Ids {
        let mut buf: Vec<u8> = Vec::new();
        IdsExporter.export(ids, &mut buf).unwrap();
        IdsImporter { name: "ids_rt" }
            .import(&DataSource::new(buf))
            .unwrap()
    }

    #[test]
    fn test_export_preserves_value_literals() {
        // Hex and decimal literals must round-trip verbatim through
        // `value_str` so callers that match on the original spelling
        // (e.g. NearInfinity-style lookups) keep working.
        let ids = Ids {
            entries: vec![
                IdsEntry {
                    value: 1,
                    value_str: "0x0001".to_owned(),
                    name: "ACID".into(),
                },
                IdsEntry {
                    value: 2,
                    value_str: "2".to_owned(),
                    name: "COLD".into(),
                },
                IdsEntry {
                    value: 8,
                    value_str: "010".to_owned(),
                    name: "OCTAL_EIGHT".into(),
                },
            ],
        };
        let rt = roundtrip(&ids);
        assert_eq!(rt, ids);
    }

    #[test]
    fn test_export_preserves_multi_token_names() {
        // ANISND.IDS-style entries have whitespace inside the name. The
        // importer captures everything after the first token, so the
        // exporter must keep the internal spacing as-is.
        let ids = Ids {
            entries: vec![IdsEntry {
                value: 0x1300,
                value_str: "0x1300".to_owned(),
                name: "MDEM     CGAMEANIMATIONTYPE_DEMOGORGON".into(),
            }],
        };
        let rt = roundtrip(&ids);
        assert_eq!(rt, ids);
    }

    #[test]
    fn test_export_empty() {
        let ids = Ids { entries: vec![] };
        let rt = roundtrip(&ids);
        assert_eq!(rt, ids);
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let ids = Ids {
            entries: vec![
                IdsEntry {
                    value: 0,
                    value_str: "0".to_owned(),
                    name: "FALSE".into(),
                },
                IdsEntry {
                    value: 1,
                    value_str: "1".to_owned(),
                    name: "TRUE".into(),
                },
            ],
        };
        let tmp = tempfile::NamedTempFile::new().unwrap();
        IdsExporter.export_to_file(&ids, tmp.path()).unwrap();
        let rt = IdsImporter {
            name: "ids_rt_file",
        }
        .import(&DataSource::new(tmp.path().to_path_buf()))
        .unwrap();
        assert_eq!(rt, ids);
    }

    #[test]
    fn test_export_all_sample_ids_files() {
        // Strongest guarantee: every shipped IDS asset round-trips through
        // import → export → import without semantic loss.
        let ids_folder = get_assets_path().join("IDS");
        let paths = get_all_in_folder_by_extension(&ids_folder, "IDS");
        assert!(!paths.is_empty(), "no IDS files found");

        for ids_path in paths {
            let original = IdsImporter { name: "ids_rt_all" }
                .import(&DataSource::new(ids_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", ids_path.display()));
            let rt = roundtrip(&original);
            assert_eq!(
                rt,
                original,
                "round-trip mismatch for {}",
                ids_path.display()
            );
        }
    }
}
