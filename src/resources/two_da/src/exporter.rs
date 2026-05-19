use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::TwoDA;

/// A 2DA file exporter.
///
/// Writes a [`TwoDA`] back to the whitespace-aligned text format used by the
/// Infinity Engine. The output is functionally — not byte-exactly —
/// equivalent to a source file: column widths, row ordering, and exact
/// spacing may differ, but re-importing the emitted text yields an equal
/// [`TwoDA`].
///
/// Layout chosen by the exporter:
/// - Line 1: `2DA V1.0` signature
/// - Line 2: default value
/// - Line 3: column headers, aligned to a key-column of width
///   `max(row_key_len) + 1` so the longest row key fits without overflowing
///   into the first value column
/// - Lines 4+: rows sorted alphabetically by key for deterministic output;
///   each cell is padded so values align with the corresponding header
///
/// Each column gets a width of `max(header_len, max_value_len_in_column) + 1`
/// so the importer's position-based parser can recover every cell.
pub struct TwoDAExporter;

impl TwoDAExporter {
    /// Writes `two_da` as 2DA text into `writer`.
    pub fn export<W: Write>(&self, two_da: &TwoDA, writer: &mut W) -> io::Result<()> {
        let n_cols = two_da.headers.len();

        // Key column width: longest row key + at least one trailing space so
        // the importer's `line[0..columns[0]].trim()` finds a clean key.
        let key_width = two_da
            .rows
            .keys()
            .map(|k| k.len())
            .max()
            .unwrap_or(0)
            .max(1)
            + 1;

        // Per-column width = max(header, any value) + 1 trailing space.
        let mut col_widths = Vec::with_capacity(n_cols);
        for (i, h) in two_da.headers.iter().enumerate() {
            let mut w = h.len();
            for values in two_da.rows.values() {
                if let Some(v) = values.get(i) {
                    w = w.max(v.len());
                }
            }
            col_widths.push(w + 1);
        }

        // 1. Signature.
        writeln!(writer, "2DA V1.0")?;

        // 2. Default value.
        writeln!(writer, "{}", two_da.default)?;

        // 3. Header line.
        let mut header_line = String::with_capacity(key_width + col_widths.iter().sum::<usize>());
        header_line.extend(std::iter::repeat_n(' ', key_width));
        for (h, w) in two_da.headers.iter().zip(col_widths.iter()) {
            header_line.push_str(h);
            header_line.extend(std::iter::repeat_n(' ', w - h.len()));
        }
        // Trim trailing spaces — the parser doesn't care, and it keeps the
        // output cleaner.
        writeln!(writer, "{}", header_line.trim_end())?;

        // 4. Rows, sorted by key for stable output.
        let mut keys: Vec<&String> = two_da.rows.keys().collect();
        keys.sort();
        for key in keys {
            let values = &two_da.rows[key];
            let mut row_line = String::with_capacity(key_width + col_widths.iter().sum::<usize>());
            row_line.push_str(key);
            row_line.extend(std::iter::repeat_n(' ', key_width - key.len()));
            for (i, w) in col_widths.iter().enumerate() {
                let v = values.get(i).map(String::as_str).unwrap_or(&two_da.default);
                row_line.push_str(v);
                row_line.extend(std::iter::repeat_n(' ', w - v.len()));
            }
            writeln!(writer, "{}", row_line.trim_end())?;
        }

        Ok(())
    }

    /// Writes `two_da` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, two_da: &TwoDA, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(two_da, &mut writer)?;
        writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TwoDAImporter;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_test_utils::get_assets_path;
    use std::collections::HashMap;

    fn two_da_eq(a: &TwoDA, b: &TwoDA) -> bool {
        a.headers == b.headers && a.default == b.default && a.rows == b.rows
    }

    fn roundtrip(two_da: &TwoDA) -> TwoDA {
        let mut buf: Vec<u8> = Vec::new();
        TwoDAExporter.export(two_da, &mut buf).unwrap();
        TwoDAImporter { name: "2da_rt" }
            .import(&DataSource::new(buf))
            .unwrap()
    }

    fn make_two_da(
        headers: &[&str],
        default: &str,
        rows: &[(&str, Vec<&str>)],
    ) -> TwoDA {
        TwoDA {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            default: default.to_string(),
            rows: rows
                .iter()
                .map(|(k, vs)| {
                    (
                        k.to_string(),
                        vs.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn test_export_basic_roundtrip() {
        let original = make_two_da(
            &["A", "B", "C"],
            "0",
            &[
                ("ROW1", vec!["1", "2", "3"]),
                ("ROW2", vec!["4", "5", "6"]),
            ],
        );
        assert!(two_da_eq(&roundtrip(&original), &original));
    }

    #[test]
    fn test_export_preserves_long_row_key() {
        // FIGHTER_MAGE_CLERIC (19 chars) is the longest key in vanilla
        // AbClasRq.2DA — the key column must widen to fit it.
        let original = make_two_da(
            &["MIN_STR", "MIN_DEX"],
            "0",
            &[
                ("FIGHTER_MAGE_CLERIC", vec!["9", "0"]),
                ("MAGE", vec!["0", "0"]),
            ],
        );
        assert!(two_da_eq(&roundtrip(&original), &original));
    }

    #[test]
    fn test_export_preserves_values_wider_than_headers() {
        // Value "12" is wider than header "A" (one char) — column must widen
        // to fit values, not just headers.
        let original = make_two_da(
            &["A", "B"],
            "0",
            &[("ROW", vec!["12345", "67890"])],
        );
        assert!(two_da_eq(&roundtrip(&original), &original));
    }

    #[test]
    fn test_export_preserves_default_used_as_filler() {
        // A row whose stored value equals `default` should round-trip
        // (whether written verbatim or elided to whitespace, both re-parse
        // to the default).
        let original = make_two_da(
            &["A", "B", "C"],
            "default",
            &[("ROW", vec!["x", "default", "y"])],
        );
        assert!(two_da_eq(&roundtrip(&original), &original));
    }

    #[test]
    fn test_export_empty_2da() {
        let original = TwoDA {
            headers: Vec::new(),
            default: "0".into(),
            rows: HashMap::new(),
        };
        assert!(two_da_eq(&roundtrip(&original), &original));
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let original = make_two_da(
            &["A", "B"],
            "0",
            &[("ROW", vec!["1", "2"])],
        );
        let tmp = tempfile::NamedTempFile::new().unwrap();
        TwoDAExporter
            .export_to_file(&original, tmp.path())
            .unwrap();
        let rt = TwoDAImporter { name: "2da_rt_file" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        assert!(two_da_eq(&rt, &original));
    }

    #[test]
    fn test_export_real_bg2_2da_roundtrip() {
        // Strongest guarantee: the shipped AbClasRq.2DA round-trips.
        let path = CaseInsensitiveFS::new(get_assets_path().join("KEY").join("bg2"))
            .unwrap()
            .get_path("override/AbClasRq.2DA")
            .unwrap();
        let original = TwoDAImporter { name: "2da_rt_real" }
            .import(&DataSource::new(path.path()))
            .unwrap();
        let rt = roundtrip(&original);
        assert!(two_da_eq(&rt, &original));
    }
}
