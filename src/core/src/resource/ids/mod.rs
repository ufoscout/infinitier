use infinitier_datasource::{DataSource, Importer};
use serde::{Deserialize, Serialize};

/// An IDS file importer.
pub struct IdsImporter;

impl Importer for IdsImporter {
    type T = Ids;

    fn import(source: &DataSource) -> std::io::Result<Ids> {
        let mut reader = source.reader()?;
        let mut entries = Vec::new();

        loop {
            let (line, bytes) = reader.read_line()?;
            if bytes == 0 {
                break;
            }
            if let Some(entry) = parse_line(line.trim()) {
                entries.push(entry);
            }
        }

        Ok(Ids { entries })
    }
}

/// An IDS file — an ordered list of integer/symbol pairs.
///
/// Insertion order is preserved; duplicate values are allowed.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ids {
    pub entries: Vec<IdsEntry>,
}

/// A single integer/symbol pair inside an IDS file.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsEntry {
    pub value: i32,
    pub value_str: String,
    pub name: String,
}

impl Ids {
    /// Returns the name of the first entry whose value matches, or `None`.
    pub fn of_value(&self, value: i32) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.value == value)
            .map(|e| e.name.as_str())
    }

    /// Returns the name of the first entry whose value string matches, or `None`.
    pub fn of_value_str(&self, value: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.value_str == value)
            .map(|e| e.name.as_str())
    }
}

/// Parse an IDS integer literal supporting decimal, hexadecimal (`0x…`), and
/// octal (`0…`), matching the `strtol(…, 0)` behaviour of the C reference.
fn parse_value(s: &str) -> Option<i32> {
    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let n: i32 = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i32::from_str_radix(hex, 16).ok()?
    } else if s.len() > 1 && s.starts_with('0') {
        i32::from_str_radix(&s[1..], 8).ok()?
    } else {
        s.parse().ok()?
    };
    Some(if neg { n.wrapping_neg() } else { n })
}

/// Parse one text line into an [`IdsEntry`].
///
/// Returns `None` for empty lines and for lines that do not contain a valid
/// `<value> <name>` pair (e.g. a bare entry count with no name).
fn parse_line(line: &str) -> Option<IdsEntry> {
    let line = line.trim().trim_matches('\0');
    if line.is_empty() {
        return None;
    }
    let (value_str, rest) = line.split_once(|c: char| c.is_ascii_whitespace())?;
    let name = rest.trim();
    if name.is_empty() {
        return None;
    }
    Some(IdsEntry {
        value: parse_value(value_str)?,
        value_str: value_str.to_owned(),
        name: name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use infinitier_datasource::DataSource;

    use crate::test_utils::RESOURCES_DIR;
    use infinitier_test_utils::{get_all_in_folder_by_extension, parse_json_file};

    // ── parse_value ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_value_decimal() {
        assert_eq!(parse_value("0"), Some(0));
        assert_eq!(parse_value("255"), Some(255));
        assert_eq!(parse_value("126"), Some(126));
    }

    #[test]
    fn test_parse_value_hex() {
        assert_eq!(parse_value("0x0001"), Some(1));
        assert_eq!(parse_value("0xFF"), Some(255));
        assert_eq!(parse_value("0X10"), Some(16));
    }

    #[test]
    fn test_parse_value_octal() {
        assert_eq!(parse_value("010"), Some(8));
        assert_eq!(parse_value("077"), Some(63));
    }

    #[test]
    fn test_parse_value_negative() {
        assert_eq!(parse_value("-1"), Some(-1));
        assert_eq!(parse_value("-255"), Some(-255));
    }

    #[test]
    fn test_parse_value_invalid() {
        assert_eq!(parse_value(""), None);
        assert_eq!(parse_value("abc"), None);
        assert_eq!(parse_value("10abc"), None);
    }

    // ── parse_line ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_line_decimal() {
        assert_eq!(
            parse_line("1 INANIMATE"),
            Some(IdsEntry {
                value: 1,
                value_str: "1".to_owned(),
                name: "INANIMATE".into()
            })
        );
    }

    #[test]
    fn test_parse_line_hex() {
        assert_eq!(
            parse_line("0x0001 ACID"),
            Some(IdsEntry {
                value: 1,
                value_str: "0x0001".to_owned(),
                name: "ACID".into()
            })
        );
    }

    #[test]
    fn test_parse_line_wide_spacing() {
        assert_eq!(
            parse_line("0          HITPOINTS"),
            Some(IdsEntry {
                value: 0,
                value_str: "0".to_owned(),
                name: "HITPOINTS".into()
            })
        );
    }

    #[test]
    fn test_parse_line_bare_count_skipped() {
        assert_eq!(parse_line("10"), None);
        assert_eq!(parse_line("5"), None);
    }

    #[test]
    fn test_parse_line_empty() {
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("   "), None);
    }

    #[test]
    fn test_parse_line_header() {
        assert_eq!(parse_line("IDS"), None);
        assert_eq!(parse_line(" IDS V1.0  "), None);
    }

    #[test]
    fn test_parse_line_multiple_values() {
        assert_eq!(
            parse_line("0x1300 MDEM     CGAMEANIMATIONTYPE_DEMOGORGON"),
            Some(IdsEntry {
                value: 4864,
                value_str: "0x1300".to_owned(),
                name: "MDEM     CGAMEANIMATIONTYPE_DEMOGORGON".into()
            })
        );
    }

    // ── IDS helper methods ───────────────────────────────────────────────────

    #[test]
    fn test_name_of() {
        let ids = Ids {
            entries: vec![
                IdsEntry {
                    value: 0,
                    value_str: "0".to_owned(),
                    name: "false".into(),
                },
                IdsEntry {
                    value: 1,
                    value_str: "1".to_owned(),
                    name: "true".into(),
                },
            ],
        };
        assert_eq!(ids.of_value(0), Some("false"));
        assert_eq!(ids.of_value(1), Some("true"));
        assert_eq!(ids.of_value(99), None);
    }

    #[test]
    fn test_all_ids_files() {
        let ids_folder = Path::new(RESOURCES_DIR).join("resources/IDS");
        let paths = get_all_in_folder_by_extension(&ids_folder, "IDS");
        assert!(!paths.is_empty(), "no IDS files found");

        for ids_path in paths {
            let json_path = ids_path.with_extension("json");
            let expected: Ids = parse_json_file(&json_path);
            let actual = IdsImporter::import(&DataSource::new(ids_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", ids_path.display()));
            assert_eq!(actual, expected, "IDS mismatch for {}", ids_path.display());
        }
    }
}
