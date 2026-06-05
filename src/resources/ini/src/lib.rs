#![doc = include_str!("../readme.md")]

use serde::{Deserialize, Serialize};

mod exporter;
mod importer;

pub use exporter::IniExporter;
pub use importer::IniImporter;

/// A parsed INI file: an ordered list of sections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ini {
    pub sections: Vec<IniSection>,
}

/// A single section inside an INI file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IniSection {
    pub name: String,
    pub entries: Vec<IniEntry>,
}

/// A single key=value entry inside a section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
