use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;

use itertools::{Itertools, chain};
use log::{debug, warn};

use infinitier_common::resource::encryption;
use infinitier_datasource::{DataSource, Importer};

use crate::TwoDA;

/// A 2DA file importer
pub struct TwoDAImporter<'a> {
    pub name: &'a str,
}

impl Importer for TwoDAImporter<'_> {
    type T = TwoDA;

    fn import(&self, source: &DataSource) -> std::io::Result<TwoDA> {
        let mut raw = Vec::new();
        source.reader()?.read_to_end(&mut raw)?;

        // Some 2DAs are stored encrypted.
        let decrypted = DataSource::new(encryption::decrypt(Cow::Owned(raw)).into_owned())
                .with_encoding(source.encoding());

        let mut reader = decrypted.reader()?;

        let signature = reader.read_line()?.0.trim().to_string();

        if signature != "2DA V1.0" {
            warn!(
                "Loaded {} [2DA] - DataSource [{:?}] has a bad signature [{signature}]! Complaining, but ignoring...",
                self.name, source
            );
        }

        let raw_default = reader.read_line()?.0.trim().to_string();
        let default = if raw_default.is_empty() {
            "0".to_string()
        } else {
            raw_default
        };
        let (headers, columns) = parse_headers(&reader.read_line()?.0);

        let mut rows = HashMap::new();
        loop {
            let (line, bytes) = reader.read_line()?;
            if bytes == 0 {
                break;
            }
            let (key, value) = parse_data_row(line.trim(), &columns, &default);
            rows.insert(key, value);
        }

        debug!("Loaded {} [2DA]: {} rows", self.name, rows.len());
        Ok(TwoDA {
            headers,
            default,
            rows,
        })
    }
}

/// Splits a string into (word, byte_start_index).
fn parse_headers(input: &str) -> (Vec<String>, Vec<usize>) {
    let mut headers = Vec::new();
    let mut columns = Vec::new();
    let mut in_word = false;
    let mut start = 0;

    for (i, c) in input.char_indices() {
        if c.is_whitespace() {
            if in_word {
                headers.push(input[start..i].to_string());
                columns.push(start);
                in_word = false;
            }
        } else if !in_word {
            start = i;
            in_word = true;
        }
    }

    if in_word {
        headers.push(input[start..].to_string());
        columns.push(start);
    }

    (headers, columns)
}

/// Parse a single row using precomputed column positions.
/// `columns` must come from `parse_headers(header_line)`.
fn parse_data_row(line: &str, columns: &[usize], default: &str) -> (String, Vec<String>) {
    let max_len = line.len();
    let key = line[0..columns[0].min(max_len)].trim().to_string();

    let mut result = Vec::with_capacity(columns.len());
    let len = &[max_len];

    let chain = chain!(columns, len);
    for (s, e) in chain.tuple_windows() {
        if s >= &max_len {
            result.push(default.to_owned());
            continue;
        }
        let word = line[*s..(*e).min(max_len)].trim();
        if word.is_empty() {
            result.push(default.to_owned());
        } else {
            result.push(word.to_string());
        }
    }

    (key, result)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_test_utils::get_assets_path;

    use super::*;

    #[test]
    fn test_split_words_simple() {
        let input = "  MIN_STR MIN_DEX   MIN_CON ";
        let (headers, columns) = parse_headers(input);

        assert_eq!(
            headers,
            vec![
                "MIN_STR".to_string(),
                "MIN_DEX".to_string(),
                "MIN_CON".to_string(),
            ]
        );

        assert_eq!(columns, vec![2, 10, 20]);
    }

    #[test]
    fn test_split_words_only_whitespace() {
        let input = "       ";
        let (headers, columns) = parse_headers(input);
        assert!(headers.is_empty());
        assert!(columns.is_empty());
    }

    #[test]
    fn test_parse_row_basic() {
        let header = "     A B C D";
        let (_, columns) = parse_headers(header);

        let row = "ROW  1 2 3 4";
        let (key, values) = parse_data_row(row, &columns, "0");

        assert_eq!(key, "ROW");
        assert_eq!(
            values,
            vec![
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_row_missing_values() {
        let header = "    A B C D";
        let (_, columns) = parse_headers(header);

        // missing C entirely
        let row = "ROW 1   2      ";
        let (key, values) = parse_data_row(row, &columns, "default");

        assert_eq!(key, "ROW");
        assert_eq!(
            values,
            vec![
                "1".to_owned(),
                "default".to_owned(),
                "2".to_owned(),
                "default".to_owned(),
            ]
        ); // defaults filled
    }

    #[test]
    fn test_parse_row_missing_all_values() {
        let header = "    A B C D";
        let (_, columns) = parse_headers(header);

        // missing C entirely
        let row = "ROW";
        let (key, values) = parse_data_row(row, &columns, "default");

        assert_eq!(key, "ROW");
        assert_eq!(
            values,
            vec![
                "default".to_owned(),
                "default".to_owned(),
                "default".to_owned(),
                "default".to_owned(),
            ]
        ); // defaults filled
    }

    #[test]
    fn test_full_processing_multiline() {
        #[rustfmt::skip]
        let text = concat!(
            "MAGE                            0       0       9       0       0\n",
            "FIGHTER                 9       0       0       0               9\n",
            "CLERIC                  0       0       0       0       9       \n",
            "THIEF                   0       9       0       0       0       0",
        );

        let lines = text.lines();

        let header = "                        MIN_STR MIN_DEX MIN_CON MIN_INT MIN_WIS MIN_CHR";
        let (_, columns) = parse_headers(header);

        let mut result = HashMap::new();

        for line in lines {
            let (key, vals) = parse_data_row(line, &columns, "1");
            result.insert(key, vals);
        }

        assert_eq!(
            result["MAGE"],
            vec![
                "1".to_string(),
                "0".to_string(),
                "0".to_string(),
                "9".to_string(),
                "0".to_string(),
                "0".to_string()
            ]
        );
        assert_eq!(
            result["FIGHTER"],
            vec![
                "9".to_string(),
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
                "1".to_string(),
                "9".to_string()
            ]
        ); // gap filled
        assert_eq!(
            result["CLERIC"],
            vec![
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
                "9".to_string(),
                "1".to_string()
            ]
        );
        assert_eq!(
            result["THIEF"],
            vec![
                "0".to_string(),
                "9".to_string(),
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
                "0".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_2da_file() {
        let path = CaseInsensitiveFS::new(get_assets_path().join("KEY").join("bg2"))
            .unwrap()
            .get_path("override/AbClasRq.2DA")
            .unwrap();
        let two_da = TwoDAImporter { name: "2da_test" }
            .import(&DataSource::new(path.path()))
            .unwrap();

        assert_eq!(
            two_da.headers,
            vec![
                "MIN_STR".to_string(),
                "MIN_DEX".to_string(),
                "MIN_CON".to_string(),
                "MIN_INT".to_string(),
                "MIN_WIS".to_string(),
                "MIN_CHR".to_string()
            ]
        );
        assert_eq!(two_da.rows.len(), 51);
        assert_eq!(two_da.default, "0");

        assert_eq!(
            two_da.rows.get("MAGE"),
            Some(&vec![
                "0".to_string(),
                "0".to_string(),
                "0".to_string(),
                "9".to_string(),
                "0".to_string(),
                "0".to_string()
            ])
        );
        assert_eq!(
            two_da.rows.get("FIGHTER_MAGE_CLERIC"),
            Some(&vec![
                "9".to_string(),
                "0".to_string(),
                "0".to_string(),
                "9".to_string(),
                "9".to_string(),
                "0".to_string()
            ])
        );
        assert_eq!(
            two_da.rows.get("PALADIN"),
            Some(&vec![
                "12".to_string(),
                "0".to_string(),
                "9".to_string(),
                "0".to_string(),
                "13".to_string(),
                "17".to_string()
            ])
        );
    }

    // ── Real-game ability 2DAs (assets/engine_caps/<key>/) ───────────
    //
    // Import the four ability-bonus tables `EngineCaps` consumes, from
    // each BG install's extracted fixtures, and assert each parsed as a
    // *proper* 2DA: it must carry at least one data row and one of the
    // expected column headers. The classic `bg` fixtures are stored
    // XOR-encrypted on disk (IE 0xFFFF signature), which this importer
    // does NOT decrypt, so `import_bg_ability_2das` is EXPECTED TO FAIL
    // until 2DA decryption is implemented.

    /// `(file, any-of expected column names)` — the same columns
    /// `EngineCaps` searches for in each table.
    const ABILITY_2DAS: &[(&str, &[&str])] = &[
        ("STRMOD.2DA", &["STR_BONUS_TO_HIT", "TO_HIT"]),
        ("STRMODEX.2DA", &["STR_BONUS_TO_HIT", "TO_HIT"]),
        ("DEXMOD.2DA", &["AC_ADJ", "ACMOD", "AC"]),
        ("HPCONBON.2DA", &["HP_BONUS", "HPCONBON", "OTHER"]),
    ];

    fn assert_ability_2das_import_properly(game_key: &str) {
        let dir = get_assets_path().join("engine_caps").join(game_key);
        let mut checked = 0;
        for (file, expected_cols) in ABILITY_2DAS {
            let path = dir.join(file);
            // Some games legitimately don't ship every table — e.g. IWD2
            // (d20) has no DEXMOD.2DA. Skip files that weren't extracted.
            if !path.exists() {
                continue;
            }
            checked += 1;
            let two_da = TwoDAImporter { name: file }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("{game_key}/{file}: import errored: {e}"));
            assert!(
                !two_da.rows.is_empty(),
                "{game_key}/{file}: parsed no data rows (likely encrypted/garbled)"
            );
            let has_col = expected_cols
                .iter()
                .any(|c| two_da.headers.iter().any(|h| h.eq_ignore_ascii_case(c)));
            assert!(
                has_col,
                "{game_key}/{file}: headers {:?} contain none of {:?} (likely encrypted/garbled)",
                two_da.headers, expected_cols
            );
            // These ability tables are score-indexed, so every row key is a
            // number. If most keys don't parse as integers the rows were
            // mis-split — e.g. the importer slices by header byte-offset but
            // the data rows are tab-delimited (IWD / IWD2).
            let numeric_keys = two_da
                .rows
                .keys()
                .filter(|k| k.trim().parse::<i64>().is_ok())
                .count();
            assert!(
                numeric_keys * 2 >= two_da.rows.len(),
                "{game_key}/{file}: only {}/{} row keys are numeric — rows likely mis-split \
                 (tab-delimited?); sample keys: {:?}",
                numeric_keys,
                two_da.rows.len(),
                two_da.rows.keys().take(3).collect::<Vec<_>>()
            );
        }
        assert!(checked > 0, "{game_key}: no ability 2DA fixtures found");
    }

    #[test]
    fn import_bg_ability_2das() {
        // EXPECTED TO FAIL: classic BG ships these 2DAs XOR-encrypted.
        assert_ability_2das_import_properly("bg");
    }

    #[test]
    fn import_bg_ee_ability_2das() {
        assert_ability_2das_import_properly("bg_ee");
    }

    #[test]
    fn import_bg2_ability_2das() {
        assert_ability_2das_import_properly("bg2");
    }

    #[test]
    fn import_bg2_ee_ability_2das() {
        assert_ability_2das_import_properly("bg2_ee");
    }

    #[test]
    fn import_iwd_ability_2das() {
        assert_ability_2das_import_properly("iwd");
    }

    #[test]
    fn import_iwd_ee_ability_2das() {
        assert_ability_2das_import_properly("iwd_ee");
    }

    #[test]
    fn import_iwd2_ability_2das() {
        // IWD2 (d20) ships no DEXMOD.2DA — the helper skips the absent file.
        assert_ability_2das_import_properly("iwd2");
    }

    #[test]
    fn import_pst_ability_2das() {
        assert_ability_2das_import_properly("pst");
    }

    #[test]
    fn import_pst_ee_ability_2das() {
        assert_ability_2das_import_properly("pst_ee");
    }
}
