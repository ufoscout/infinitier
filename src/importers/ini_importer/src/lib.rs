#![doc = include_str!("../readme.md")]

use infinitier_datasource::{DataSource, Importer};
use log::debug;
use serde::{Deserialize, Serialize};

/// An INI file importer.
///
/// Follows the parsing rules of GemRB's INIImporter:
/// - Lines starting with `;` are comments and are skipped.
/// - `[section]` starts a new section; any text after the closing `]` is ignored.
/// - Key and value are each trimmed of leading/trailing whitespace.
/// - Empty-value entries (`key=`) are stored with an empty string value.
/// - Section and key lookups are case-insensitive.
pub struct IniImporter<'a> {
    pub name: &'a str,
}

impl<'a> Importer for IniImporter<'a> {
    type T = Ini;

    fn import(&self, source: &DataSource) -> std::io::Result<Ini> {
        let mut reader = source.reader()?;
        let mut sections: Vec<IniSection> = Vec::new();

        loop {
            let (line, bytes) = reader.read_line()?;
            if bytes == 0 {
                break;
            }
            let line = line.trim_end_matches(['\r', '\n', '\0']);

            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') {
                let end = line.find(']').unwrap_or(line.len());
                let name = line[1..end].to_string();
                sections.push(IniSection {
                    name,
                    entries: Vec::new(),
                });
                continue;
            }

            // Strip inline // comments from entry lines (Infinity Engine INI format
            // uses // instead of ; for comments, per NearInfinity's IniMap).
            let line = match line.find("//") {
                Some(p) => line[..p].trim_end(),
                None => line,
            };
            if line.is_empty() {
                continue;
            }

            if let Some(eq) = line.find('=')
                && let Some(section) = sections.last_mut()
            {
                let key = line[..eq].trim().to_string();
                let value = line[eq + 1..].trim().to_string();
                if !key.is_empty() {
                    section.entries.push(IniEntry { key, value });
                }
            }
        }

        debug!("Loaded {} [INI]: {} sections", self.name, sections.len());
        Ok(Ini { sections })
    }
}

/// A parsed INI file: an ordered list of sections.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Ini {
    pub sections: Vec<IniSection>,
}

/// A single section inside an INI file.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct IniSection {
    pub name: String,
    pub entries: Vec<IniEntry>,
}

/// A single key=value entry inside a section.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct IniEntry {
    pub key: String,
    /// Raw string value; empty when the file had `key=` with nothing after it.
    pub value: String,
}

impl Ini {
    /// Find a section by name (case-insensitive).
    pub fn section(&self, name: &str) -> Option<&IniSection> {
        self.sections
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Retrieve a value by section and key (both case-insensitive).
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section)?.get(key)
    }
}

impl IniSection {
    /// Find a value by key (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key.eq_ignore_ascii_case(key))
            .map(|e| e.value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path, parse_json_file};

    fn ini_folder() -> std::path::PathBuf {
        get_assets_path().join("INI")
    }

    fn parse(src: &str) -> Ini {
        IniImporter { name: "ini_test" }
            .import(&DataSource::new(src.as_bytes()))
            .unwrap()
    }

    #[test]
    fn test_basic_section_and_entry() {
        let ini = parse("[section]\nkey=value\n");
        assert_eq!(ini.sections.len(), 1);
        assert_eq!(ini.sections[0].name, "section");
        assert_eq!(ini.sections[0].entries[0].key, "key");
        assert_eq!(ini.sections[0].entries[0].value, "value");
    }

    #[test]
    fn test_section_name_ignores_trailing_comment() {
        let ini = parse("[MANI] /animated plate/\natt1=sound\n");
        assert_eq!(ini.sections[0].name, "MANI");
    }

    #[test]
    fn test_semicolon_comment_skipped() {
        let ini = parse("; this is a comment\n[s]\nk=v\n");
        assert_eq!(ini.sections.len(), 1);
        assert_eq!(ini.sections[0].entries[0].key, "k");
    }

    #[test]
    fn test_empty_section_kept() {
        let ini = parse("[empty]\n[real]\nk=v\n");
        assert_eq!(ini.sections.len(), 2);
        assert_eq!(ini.sections[0].name, "empty");
        assert!(ini.sections[0].entries.is_empty());
        assert_eq!(ini.sections[1].name, "real");
    }

    #[test]
    fn test_empty_value_stored() {
        let ini = parse("[s]\natt1=\n");
        assert_eq!(ini.sections[0].entries[0].key, "att1");
        assert_eq!(ini.sections[0].entries[0].value, "");
    }

    #[test]
    fn test_get_returns_empty_string_for_empty_value() {
        let ini = parse("[s]\natt1=\n");
        assert_eq!(ini.get("s", "att1"), Some(""));
    }

    #[test]
    fn test_whitespace_trimmed_from_key_and_value() {
        let ini = parse("[s]\n  key  =  value  \n");
        assert_eq!(ini.sections[0].entries[0].key, "key");
        assert_eq!(ini.sections[0].entries[0].value, "value");
    }

    #[test]
    fn test_case_insensitive_section_lookup() {
        let ini = parse("[Section]\nk=v\n");
        assert!(ini.section("section").is_some());
        assert!(ini.section("SECTION").is_some());
        assert!(ini.section("SeCTiOn").is_some());
    }

    #[test]
    fn test_case_insensitive_key_lookup() {
        let ini = parse("[s]\nMyKey=hello\n");
        assert_eq!(ini.get("s", "mykey"), Some("hello"));
        assert_eq!(ini.get("s", "MYKEY"), Some("hello"));
    }

    #[test]
    fn test_multiple_sections() {
        let ini = parse("[a]\nx=1\n[b]\ny=2\n");
        assert_eq!(ini.sections.len(), 2);
        assert_eq!(ini.get("a", "x"), Some("1"));
        assert_eq!(ini.get("b", "y"), Some("2"));
    }

    #[test]
    fn test_double_slash_standalone_comment_skipped() {
        let ini = parse("[s]\n// this is a comment\nk=v\n");
        assert_eq!(ini.sections[0].entries.len(), 1);
        assert_eq!(ini.sections[0].entries[0].key, "k");
    }

    #[test]
    fn test_double_slash_inline_comment_stripped_from_value() {
        let ini = parse("[s]\nk=hello // world\n");
        assert_eq!(ini.sections[0].entries[0].value, "hello");
    }

    #[test]
    fn test_double_slash_before_equals_drops_entry() {
        // // before = means the whole meaningful part is a comment → entry is dropped
        let ini = parse("[s]\nkey // = value\n");
        assert!(ini.sections[0].entries.is_empty());
    }

    #[test]
    fn test_section_with_only_empty_values_is_kept() {
        let ini = parse("[s]\natt1=\natt2=\n");
        assert_eq!(ini.sections.len(), 1);
        assert_eq!(ini.sections[0].entries.len(), 2);
    }

    #[test]
    fn test_all_ini_files() {
        let paths = get_all_in_folder_by_extension(ini_folder(), "ini");
        assert!(!paths.is_empty(), "no INI files found");

        for ini_path in paths {
            let json_path = ini_path.with_extension("json");
            let expected: Ini = parse_json_file(&json_path);
            let actual = IniImporter { name: "ini_test" }
                .import(&DataSource::new(ini_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", ini_path.display()));
            assert_eq!(actual, expected, "INI mismatch for {}", ini_path.display());
        }
    }
}
