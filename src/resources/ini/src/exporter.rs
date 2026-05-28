use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::Ini;

/// An INI file exporter.
///
/// Writes an [`Ini`] back to the textual INI format used by the Infinity
/// Engine. The output is functionally — not byte-exactly — equivalent to a
/// source file: column alignment, comments, and inter-section spacing are
/// not preserved, but re-importing the emitted text yields an equal [`Ini`].
///
/// Sections are emitted in their stored order as `[name]` followed by each
/// entry as `key=value`. A blank line separates sections for readability.
pub struct IniExporter;

impl IniExporter {
    /// Writes `ini` as INI text into `writer`.
    pub fn export<W: Write>(&self, ini: &Ini, writer: &mut W) -> io::Result<()> {
        for (i, section) in ini.sections.iter().enumerate() {
            if i > 0 {
                writeln!(writer)?;
            }
            writeln!(writer, "[{}]", section.name)?;
            for entry in &section.entries {
                writeln!(writer, "{}={}", entry.key, entry.value)?;
            }
        }
        Ok(())
    }

    /// Writes `ini` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, ini: &Ini, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(ini, &mut writer)?;
        writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IniEntry, IniImporter, IniSection};
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path};

    fn roundtrip(ini: &Ini) -> Ini {
        let mut buf: Vec<u8> = Vec::new();
        IniExporter.export(ini, &mut buf).unwrap();
        IniImporter { name: "ini_rt" }
            .import(&DataSource::new(buf))
            .unwrap()
    }

    #[test]
    fn test_export_basic() {
        let ini = Ini {
            sections: vec![IniSection {
                name: "section".into(),
                entries: vec![IniEntry {
                    key: "key".into(),
                    value: "value".into(),
                }],
            }],
        };
        assert_eq!(roundtrip(&ini), ini);
    }

    #[test]
    fn test_export_preserves_empty_values() {
        // The Bioware "sounds.ini"-style entries (att1=, att1frame=) must
        // survive the round-trip with their empty values intact.
        let ini = Ini {
            sections: vec![IniSection {
                name: "MANI".into(),
                entries: vec![
                    IniEntry {
                        key: "att1".into(),
                        value: "".into(),
                    },
                    IniEntry {
                        key: "att1frame".into(),
                        value: "".into(),
                    },
                ],
            }],
        };
        assert_eq!(roundtrip(&ini), ini);
    }

    #[test]
    fn test_export_preserves_empty_section() {
        let ini = Ini {
            sections: vec![
                IniSection {
                    name: "locals".into(),
                    entries: vec![],
                },
                IniSection {
                    name: "real".into(),
                    entries: vec![IniEntry {
                        key: "k".into(),
                        value: "v".into(),
                    }],
                },
            ],
        };
        assert_eq!(roundtrip(&ini), ini);
    }

    #[test]
    fn test_export_preserves_values_with_brackets_and_dots() {
        // AR8013.ini-style values like `[255.0.0.0.101]` and
        // `[0150.0950:12]` must not be confused with section headers.
        // The importer never treats non-`[`-prefixed lines as sections, so
        // this is safe by construction — but it's worth pinning down.
        let ini = Ini {
            sections: vec![IniSection {
                name: "SPAWN1A".into(),
                entries: vec![
                    IniEntry {
                        key: "spec".into(),
                        value: "[255.0.0.0.101]".into(),
                    },
                    IniEntry {
                        key: "spawn_point".into(),
                        value: "[0150.0950:12]".into(),
                    },
                ],
            }],
        };
        assert_eq!(roundtrip(&ini), ini);
    }

    #[test]
    fn test_export_empty_ini() {
        let ini = Ini { sections: vec![] };
        let mut buf: Vec<u8> = Vec::new();
        IniExporter.export(&ini, &mut buf).unwrap();
        assert!(buf.is_empty());
        assert_eq!(roundtrip(&ini), ini);
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let ini = Ini {
            sections: vec![IniSection {
                name: "s".into(),
                entries: vec![IniEntry {
                    key: "k".into(),
                    value: "v".into(),
                }],
            }],
        };
        let tmp = tempfile::NamedTempFile::new().unwrap();
        IniExporter.export_to_file(&ini, tmp.path()).unwrap();
        let rt = IniImporter {
            name: "ini_rt_file",
        }
        .import(&DataSource::new(tmp.path().to_path_buf()))
        .unwrap();
        assert_eq!(rt, ini);
    }

    #[test]
    fn test_export_all_sample_ini_files() {
        // Strongest guarantee: every shipped INI asset round-trips through
        // import → export → import without semantic loss.
        let ini_folder = get_assets_path().join("INI");
        let paths = get_all_in_folder_by_extension(&ini_folder, "ini", false);
        assert!(!paths.is_empty(), "no INI files found");

        for ini_path in paths {
            let original = IniImporter { name: "ini_rt_all" }
                .import(&DataSource::new(ini_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", ini_path.display()));
            let rt = roundtrip(&original);
            assert_eq!(
                rt,
                original,
                "round-trip mismatch for {}",
                ini_path.display()
            );
        }
    }
}
