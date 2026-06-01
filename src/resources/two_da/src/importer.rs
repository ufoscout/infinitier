use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;

use log::debug;

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

        // Line 0 is a free-form signature/title that the Infinity Engine
        // does not enforce: shipped 2DAs use "2DA V1.0", odd-whitespace
        // variants ("2DA      V1.0", "2DA\tV1.0"), "2DA V1.1", bare "2DA",
        // and the legacy creature/animation tables even put a title there
        // ("Mbeh/beholder"). NearInfinity ignores it by default too, so we
        // just consume the line without validating.
        let _signature = reader.read_line()?;

        let raw_default = reader.read_line()?.0.trim().to_string();
        let default = if raw_default.is_empty() {
            "0".to_string()
        } else {
            raw_default
        };
        let headers = parse_headers(&reader.read_line()?.0);
        let n_cols = headers.len();

        let mut rows = HashMap::new();
        loop {
            let (line, bytes) = reader.read_line()?;
            if bytes == 0 {
                break;
            }
            if let Some((key, value)) = parse_data_row(&line, n_cols, &default) {
                rows.insert(key, value);
            }
        }

        debug!("Loaded {} [2DA]: {} rows", self.name, rows.len());
        Ok(TwoDA {
            headers,
            default,
            rows,
        })
    }
}

/// Parse the header line into its column names.
///
/// The Infinity Engine 2DA format is whitespace-delimited: any run of
/// spaces and/or tabs separates fields. So the headers are simply the
/// whitespace-separated tokens of the third line.
fn parse_headers(line: &str) -> Vec<String> {
    line.split_ascii_whitespace().map(str::to_string).collect()
}

/// Parse one data row. The first whitespace-separated token is the row
/// key; the remaining tokens are the cell values, mapped positionally to
/// the `n_cols` columns. Returns `None` for blank lines.
///
/// Cells are separated by *runs* of whitespace (spaces, tabs, or a mix),
/// so we tokenise the line rather than slice it at fixed header offsets —
/// the offset approach breaks on the tab-delimited and mixed-spacing
/// files that ship across the games (NearInfinity tokenises the same
/// way). A row with fewer values than columns is padded with `default`;
/// any extra values beyond `n_cols` are dropped, so every row ends up
/// exactly `n_cols` wide. Because whitespace runs collapse, an *empty
/// interior* cell can't be represented — but real 2DA files always fill
/// every cell (with `0`, `*`, the default literal, …), so missing values
/// only ever occur at the end of a row.
fn parse_data_row(line: &str, n_cols: usize, default: &str) -> Option<(String, Vec<String>)> {
    let mut tokens = line.split_ascii_whitespace();
    let key = tokens.next()?.to_string();
    let mut values: Vec<String> = tokens.map(str::to_string).collect();
    values.resize(n_cols, default.to_string());
    Some((key, values))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use infinitier_fs::CaseInsensitiveFS;
    use infinitier_test_utils::get_assets_path;

    use super::*;

    #[test]
    fn parse_headers_splits_on_whitespace_runs() {
        assert_eq!(
            parse_headers("  MIN_STR MIN_DEX   MIN_CON "),
            vec!["MIN_STR", "MIN_DEX", "MIN_CON"]
        );
        // Tabs and mixed tab/space runs are equally valid separators.
        assert_eq!(parse_headers("\tA\t B  \tC"), vec!["A", "B", "C"]);
    }

    #[test]
    fn parse_headers_empty_when_only_whitespace() {
        assert!(parse_headers("       ").is_empty());
        assert!(parse_headers("\t \t").is_empty());
    }

    #[test]
    fn parse_row_space_tab_and_mixed_delimiters() {
        let expected = Some((
            "ROW".to_string(),
            vec![
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string(),
            ],
        ));
        // Space-aligned, tab-delimited, and mixed all tokenise the same.
        assert_eq!(parse_data_row("ROW  1 2 3 4", 4, "0"), expected);
        assert_eq!(parse_data_row("ROW\t1\t2\t3\t4", 4, "0"), expected);
        assert_eq!(parse_data_row("ROW \t1  2\t 3   4", 4, "0"), expected);
    }

    #[test]
    fn parse_row_pads_trailing_missing_with_default() {
        assert_eq!(
            parse_data_row("ROW 1 2", 4, "default"),
            Some((
                "ROW".to_string(),
                vec![
                    "1".to_string(),
                    "2".to_string(),
                    "default".to_string(),
                    "default".to_string(),
                ]
            ))
        );
    }

    #[test]
    fn parse_row_key_only_is_all_default() {
        assert_eq!(
            parse_data_row("ROW", 4, "default"),
            Some((
                "ROW".to_string(),
                vec![
                    "default".to_string(),
                    "default".to_string(),
                    "default".to_string(),
                    "default".to_string(),
                ]
            ))
        );
    }

    #[test]
    fn parse_row_truncates_extra_values_to_columns() {
        assert_eq!(
            parse_data_row("ROW 1 2 3 4 5", 3, "0"),
            Some((
                "ROW".to_string(),
                vec!["1".to_string(), "2".to_string(), "3".to_string()]
            ))
        );
    }

    #[test]
    fn parse_row_blank_line_is_skipped() {
        assert_eq!(parse_data_row("", 4, "0"), None);
        assert_eq!(parse_data_row("   \t ", 4, "0"), None);
    }

    #[test]
    fn full_processing_handles_space_and_tab_rows() {
        let header = "        MIN_STR MIN_DEX MIN_CON";
        let n = parse_headers(header).len();
        let mut result = HashMap::new();
        // A space-aligned row, a tab-delimited row, and a mixed one.
        for line in ["MAGE   3 3 3", "FIGHTER\t9\t3\t9", "THIEF 6  9\t3"] {
            if let Some((key, vals)) = parse_data_row(line, n, "1") {
                result.insert(key, vals);
            }
        }
        assert_eq!(result["MAGE"], vec!["3", "3", "3"]);
        assert_eq!(result["FIGHTER"], vec!["9", "3", "9"]);
        assert_eq!(result["THIEF"], vec!["6", "9", "3"]);
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

    // ── Varied real 2DAs (assets/2DA/) ───────────────────────────────
    //
    // Files extracted from the games covering the formatting variety the
    // whitespace tokeniser must handle: space-aligned, tab-delimited,
    // mixed tab/space, and 3 / 4 / 5 / 12 / 25 columns. All must parse to
    // the correct headers, row count, and cell values regardless of how
    // the source happens to be whitespace-formatted.

    fn import_2da_fixture(name: &str) -> TwoDA {
        let path = get_assets_path().join("2DA").join(name);
        TwoDAImporter { name }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("import {name}: {e}"))
    }

    #[test]
    fn fixture_space_aligned_4_columns() {
        let t = import_2da_fixture("space_4col.2da");
        assert_eq!(
            t.headers,
            [
                "TO_HIT",
                "DAMAGE",
                "BEND_BARS_LIFT_GATES",
                "WEIGHT_ALLOWANCE"
            ]
        );
        assert_eq!(t.rows.len(), 26);
        assert_eq!(t.rows["0"], ["-20", "-20", "0", "0"]);
        assert_eq!(t.rows["1"], ["-5", "-4", "1", "1"]);
    }

    #[test]
    fn fixture_tab_delimited_4_columns() {
        let t = import_2da_fixture("tab_4col.2da");
        assert_eq!(
            t.headers,
            [
                "TO_HIT",
                "DAMAGE",
                "BEND_BARS_LIFT_GATES",
                "WEIGHT_ALLOWANCE"
            ]
        );
        assert_eq!(t.rows.len(), 41);
        assert_eq!(t.rows["0"], ["-20", "-20", "0", "0"]);
        assert_eq!(t.rows["1"], ["-5", "-5", "0", "1"]);
    }

    #[test]
    fn fixture_three_columns() {
        let t = import_2da_fixture("three_col.2da");
        assert_eq!(t.headers, ["REACTION", "MISSILE", "AC"]);
        assert_eq!(t.rows.len(), 26);
        assert_eq!(t.rows["0"], ["-20", "-20", "5"]);
        assert_eq!(t.rows["1"], ["-6", "-6", "5"]);
    }

    #[test]
    fn fixture_five_columns() {
        let t = import_2da_fixture("five_col.2da");
        assert_eq!(
            t.headers,
            [
                "OTHER",
                "WARRIOR",
                "MIN_ROLL",
                "REGENERATION_RATE",
                "FATIGUE_BONUS"
            ]
        );
        assert_eq!(t.rows.len(), 25);
        assert_eq!(t.rows["1"], ["-3", "-3", "1", "0", "-4"]);
        assert_eq!(t.rows["2"], ["-2", "-2", "1", "0", "-3"]);
    }

    #[test]
    fn fixture_mixed_delimiters_25_columns() {
        let t = import_2da_fixture("mixed.2da");
        assert_eq!(t.headers.len(), 25);
        assert_eq!(t.headers[0], "ALORA");
        assert_eq!(t.rows.len(), 25);
        // Every row is normalised to exactly the column count.
        assert!(t.rows.values().all(|v| v.len() == 25));
        // MINSC's value in the DYNAHEIR column (index 2) is 1.
        assert_eq!(t.rows["MINSC"][2], "1");
    }

    #[test]
    fn fixture_wide_12_columns() {
        let t = import_2da_fixture("wide.2da");
        assert_eq!(t.headers.len(), 12);
        assert_eq!(t.headers[0], "MIN_STR");
        assert_eq!(t.rows.len(), 7);
        assert_eq!(
            t.rows["HUMAN"],
            [
                "3", "18", "3", "18", "3", "18", "3", "18", "3", "18", "3", "18"
            ]
        );
        assert_eq!(
            t.rows["DWARF"],
            [
                "8", "18", "3", "18", "11", "18", "3", "18", "3", "18", "3", "18"
            ]
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
