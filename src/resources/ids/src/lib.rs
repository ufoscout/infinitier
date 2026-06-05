#![doc = include_str!("../readme.md")]

use serde::{Deserialize, Serialize};

mod exporter;
mod importer;

pub use exporter::IdsExporter;
pub use importer::IdsImporter;

/// An IDS file — an ordered list of integer/symbol pairs.
///
/// Insertion order is preserved; duplicate values are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ids {
    pub entries: Vec<IdsEntry>,
}

/// A single integer/symbol pair inside an IDS file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
