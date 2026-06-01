use std::borrow::Cow;
use std::io::Read;

use infinitier_common::resource::encryption;
use infinitier_datasource::{DataSource, Importer};
use log::debug;

use crate::{Ids, IdsEntry};

/// An IDS file importer.
pub struct IdsImporter<'a> {
    pub name: &'a str,
}

impl Importer for IdsImporter<'_> {
    type T = Ids;

    fn import(&self, source: &DataSource) -> std::io::Result<Ids> {

        let mut raw = Vec::new();
        source.reader()?.read_to_end(&mut raw)?;
        
        // Some IDs files are stored encrypted.
        let decrypted = DataSource::new(encryption::decrypt(Cow::Owned(raw)).into_owned())
                .with_encoding(source.encoding());

        let mut reader = decrypted.reader()?;
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

        debug!("Loaded {} [IDS]: {} entries", self.name, entries.len());
        Ok(Ids { entries })
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
    // Strip inline // comments, matching NearInfinity's extractIDS behaviour.
    let rest = match rest.find("//") {
        Some(p) => &rest[..p],
        None => rest,
    };
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
    use super::*;
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path, parse_json_file};

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
    fn test_parse_line_inline_comment_stripped() {
        assert_eq!(
            parse_line("5 SOMENAME // this is a comment"),
            Some(IdsEntry {
                value: 5,
                value_str: "5".to_owned(),
                name: "SOMENAME".into()
            })
        );
        assert_eq!(
            parse_line("1 TRIGGER//no space before comment"),
            Some(IdsEntry {
                value: 1,
                value_str: "1".to_owned(),
                name: "TRIGGER".into()
            })
        );
        // line with only a comment after value: name becomes empty → skip
        assert_eq!(parse_line("3 // only a comment"), None);
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

    #[test]
    fn test_all_ids_files() {
        let ids_folder = get_assets_path().join("IDS");
        let paths = get_all_in_folder_by_extension(&ids_folder, "IDS", false);
        assert!(!paths.is_empty(), "no IDS files found");

        for ids_path in paths {
            let json_path = ids_path.with_extension("json");
            let expected: Ids = parse_json_file(&json_path);
            let actual = IdsImporter { name: "ids_test" }
                .import(&DataSource::new(ids_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", ids_path.display()));
            assert_eq!(actual, expected, "IDS mismatch for {}", ids_path.display());
        }
    }

    // ── Encrypted IDS (assets/IDS/encrypted/<game>/) ─────────────────
    //
    // Many stock IDS files are stored XOR-encrypted (IE 0xFFFF). The
    // importer must transparently decrypt them. Each fixture below was
    // extracted, still-encrypted, from a real install; importing it must
    // yield a sensible table (multiple entries, all with plain-ASCII
    // names). Before decryption support, these FAIL (the garbled bytes
    // parse to ~no entries).

    fn assert_encrypted_ids_decode(game_key: &str) {
        let dir = get_assets_path()
            .join("IDS")
            .join("encrypted")
            .join(game_key);
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}")) {
            let path = entry.unwrap().path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ids"))
                != Some(true)
            {
                continue;
            }
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            let ids = IdsImporter { name: &file }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("{game_key}/{file}: import errored: {e}"));
            assert!(
                ids.entries.len() >= 2,
                "{game_key}/{file}: only {} entries parsed — likely still encrypted",
                ids.entries.len()
            );
            for e in &ids.entries {
                assert!(
                    !e.name.is_empty()
                        && e.name
                            .chars()
                            .all(|c| c.is_ascii_graphic() || c == ' ' || c == '\t'),
                    "{game_key}/{file}: garbled entry name {:?} — likely still encrypted",
                    e.name
                );
            }
            checked += 1;
        }
        assert!(checked > 0, "{game_key}: no encrypted IDS fixtures found");
    }

    #[test]
    fn import_encrypted_ids_bg() {
        assert_encrypted_ids_decode("bg");
    }

    #[test]
    fn import_encrypted_ids_bg_ee() {
        assert_encrypted_ids_decode("bg_ee");
    }

    #[test]
    fn import_encrypted_ids_bg2() {
        assert_encrypted_ids_decode("bg2");
    }

    #[test]
    fn import_encrypted_ids_bg2_ee() {
        assert_encrypted_ids_decode("bg2_ee");
    }

    #[test]
    fn import_encrypted_ids_iwd_ee() {
        assert_encrypted_ids_decode("iwd_ee");
    }
}
