use std::collections::HashMap;

use itertools::{Itertools, chain};
use log::warn;

use crate::datasource::{DataSource, Importer};

/// A 2DA file importer
pub struct TwoDAImporter;

impl Importer for TwoDAImporter {
    type T = TwoDA;

    fn import(source: &DataSource) -> std::io::Result<TwoDA> {
        let mut reader = source.reader()?;

        let signature = reader.read_line()?.0.trim().to_string();

        if signature != "2DA V1.0" {
            warn!(
                "TwoDAImporter - DataSource [{:?}] has a bad signature [{signature}]! Complaining, but ignoring...",
                source
            );
        }

        let default_value = reader.read_line()?.0.trim().to_string();
        let (headers, columns) = parse_headers(&reader.read_line()?.0);

        let mut rows = HashMap::new();
        loop {
            let (line, bytes) = reader.read_line()?;
            if bytes == 0 {
                break;
            }
            let (key, value) = parse_data_row(line.trim(), &columns, &default_value);
            rows.insert(key, value);
        }

        Ok(TwoDA {
            headers,
            columns,
            rows,
        })
    }
}

/// Represents a 2DA file.
pub struct TwoDA {
    pub headers: Vec<String>,
    pub columns: Vec<usize>,
    pub rows: HashMap<String, Vec<String>>,
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
        } else {
            if !in_word {
                start = i;
                in_word = true;
            }
        }
    }

    if in_word {
        headers.push(input[start..].to_string());
        columns.push(start);
    }

    (headers, columns)
}

/// Parse a single row using precomputed column positions.
/// `columns` must come from `split_words_with_positions(header_line)`.
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
        let word = line[*s..*e].trim();
        if word.is_empty() {
            result.push(default.to_owned());
        } else {
            result.push(word.to_string());
        }
    }

    (key, result)
}

//////////////////////////////////////////////////////////////////////
//                           UNIT TESTS                             //
//////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        fs::{CaseInsensitiveFS, CaseInsensitivePath},
        test_utils::BG2_RESOURCES_DIR,
    };

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
        let text = "MAGE                            0       0       9       0       0
FIGHTER                 9       0       0       0               9
CLERIC                  0       0       0       0       9       
THIEF                   0       9       0       0       0       0";

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
        let path = CaseInsensitiveFS::new(BG2_RESOURCES_DIR)
            .unwrap()
            .get_path(&CaseInsensitivePath::new("override/AbClasRq.2DA"))
            .unwrap();
        let two_da = TwoDAImporter::import(&DataSource::new(path)).unwrap();

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
        assert_eq!(two_da.columns, vec![24, 32, 40, 48, 56, 64]);
        assert_eq!(two_da.rows.len(), 51);

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
}
