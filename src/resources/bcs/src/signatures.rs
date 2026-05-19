//! Trigger / action function signatures parsed from `TRIGGER.IDS` and `ACTION.IDS`.
//!
//! Each entry in those IDS files maps a function id to a signature like
//! `Acquired(S:ResRef*)` or `0x4051 HaveSpell(I:Spell*Spell)`. This module turns
//! that text into a structured form so the BAF decompiler can render
//! `<Name>(<arg>, <arg>, ...)` from a parsed BCS trigger / action.

use std::collections::HashMap;

use infinitier_ids_resource::Ids;

/// Parsed signatures for a function table (TRIGGER.IDS or ACTION.IDS).
///
/// The same numeric id can map to several signatures; resolution at decompile
/// time picks the one whose parameter shape best fits the actual values.
#[derive(Debug, Clone, Default)]
pub struct Signatures {
    by_id: HashMap<i32, Vec<Function>>,
    /// Lower-cased name → `(id, index in by_id[id])`. Avoids cloning every
    /// `Function` into both maps; name lookups indirect through `by_id`.
    by_name: HashMap<String, (i32, usize)>,
}

/// One function definition as parsed from an IDS line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub id: i32,
    pub name: String,
    pub params: Vec<Parameter>,
}

/// One parameter inside a function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub kind: ParamKind,
    pub name: String,
    /// Lower-cased IDS resource name referenced by this parameter (without
    /// extension); empty when no IDS lookup is requested.
    pub ids_ref: String,
}

/// Parameter type letters used in TRIGGER.IDS / ACTION.IDS signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// `A` — nested action (only the `ActionOverride` slot uses this).
    Action,
    /// `T` — nested trigger (only the `TriggerOverride` slot uses this).
    Trigger,
    /// `I` — numeric parameter.
    Integer,
    /// `O` — object specifier.
    Object,
    /// `P` — point structure.
    Point,
    /// `S` — string parameter.
    String,
}

impl Signatures {
    /// Builds a [`Signatures`] table from a parsed IDS file.
    ///
    /// Matches NearInfinity's deduplication: when several IDS lines share the
    /// same id *and* the same parameter shape (e.g. `8 Dialogue(O:Object*)`
    /// followed by `8 Dialog(O:Object*)`), only the first one is kept.
    /// Functions sharing an id but having different parameter shapes (e.g.
    /// `7 CreateCreature(...)` vs `7 CreateCreatureEffect(...)`) are all
    /// retained; the BAF decompiler picks among them by param-shape scoring.
    pub fn from_ids(ids: &Ids) -> Self {
        let mut by_id: HashMap<i32, Vec<Function>> = HashMap::new();
        let mut by_name: HashMap<String, (i32, usize)> = HashMap::new();
        for entry in &ids.entries {
            let Some(func) = Function::parse(entry.value, &entry.name) else {
                continue;
            };
            let func_id = func.id;
            let bucket = by_id.entry(func_id).or_default();
            // NI's Function.equals ignores the name and only compares id +
            // params + type, so two same-id same-shape definitions collide and
            // the first added wins. Mirror that here.
            if bucket.iter().any(|existing| existing.params == func.params) {
                continue;
            }
            let idx = bucket.len();
            let lc_name = func.name.to_ascii_lowercase();
            bucket.push(func);
            by_name.entry(lc_name).or_insert((func_id, idx));
        }
        Self { by_id, by_name }
    }

    /// Returns the signatures registered for a given numeric id, if any.
    pub fn get(&self, id: i32) -> Option<&[Function]> {
        self.by_id.get(&id).map(|v| v.as_slice())
    }

    /// Returns a function by name (case-insensitive).
    pub fn get_by_name(&self, name: &str) -> Option<&Function> {
        let (id, idx) = self.by_name.get(&name.to_ascii_lowercase()).copied()?;
        self.by_id.get(&id).and_then(|v| v.get(idx))
    }

    /// Adds a function signature parsed from raw text (e.g.
    /// `"Clicked(O:Object*)"`). Used to patch in functions that NearInfinity
    /// hardcodes for specific games but that aren't in their TRIGGER.IDS /
    /// ACTION.IDS — for example, PST is missing `0x4070 Clicked(O:Object*)`.
    /// Returns `true` if added, `false` if a same-shape definition already
    /// exists for `id`.
    pub fn add_function(&mut self, id: i32, signature: &str) -> bool {
        let Some(func) = Function::parse(id, signature) else {
            return false;
        };
        let bucket = self.by_id.entry(id).or_default();
        if bucket.iter().any(|existing| existing.params == func.params) {
            return false;
        }
        let idx = bucket.len();
        let lc_name = func.name.to_ascii_lowercase();
        bucket.push(func);
        self.by_name.entry(lc_name).or_insert((id, idx));
        true
    }
}

impl Function {
    /// Parses a function signature like `Acquired(S:ResRef*)` paired with its id.
    /// Returns `None` when the line cannot be parsed (e.g. a bare entry count,
    /// missing parentheses, …).
    pub fn parse(id: i32, signature: &str) -> Option<Self> {
        let signature = signature.trim();
        let open = signature.find('(')?;
        let close = signature.rfind(')')?;
        if close < open {
            return None;
        }
        let name = signature[..open].trim().to_string();
        if name.is_empty() {
            return None;
        }
        let params = parse_params(&signature[open + 1..close]);
        Some(Function { id, name, params })
    }
}

fn parse_params(s: &str) -> Vec<Parameter> {
    // Each parameter follows `T:Name*IdsRef` where T is a single letter
    // ([AIOPST]) and the ids reference is optional. Parameters are
    // comma-separated and may be padded with whitespace.
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b',') {
            i += 1;
        }
        if i + 1 >= bytes.len() || bytes[i + 1] != b':' {
            break;
        }
        let kind = match (bytes[i] as char).to_ascii_uppercase() {
            'A' => ParamKind::Action,
            'T' => ParamKind::Trigger,
            'I' => ParamKind::Integer,
            // '0' is a known typo for 'O' in some IDS files (matches NI behaviour).
            'O' | '0' => ParamKind::Object,
            'P' => ParamKind::Point,
            'S' => ParamKind::String,
            _ => {
                i += 2;
                continue;
            }
        };
        i += 2;
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'*' && bytes[i] != b',' && bytes[i] != b')' {
            i += 1;
        }
        let name: String = std::str::from_utf8(&bytes[name_start..i])
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let mut ids_ref = String::new();
        if i < bytes.len() && bytes[i] == b'*' {
            i += 1;
            let ref_start = i;
            while i < bytes.len()
                && bytes[i] != b' '
                && bytes[i] != b'\t'
                && bytes[i] != b','
                && bytes[i] != b')'
            {
                i += 1;
            }
            ids_ref = std::str::from_utf8(&bytes[ref_start..i])
                .unwrap_or("")
                .to_ascii_lowercase();
        }
        out.push(Parameter {
            kind,
            name,
            ids_ref,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_ids_resource::IdsEntry;

    fn ids(entries: &[(i32, &str, &str)]) -> Ids {
        Ids {
            entries: entries
                .iter()
                .map(|(v, vs, n)| IdsEntry {
                    value: *v,
                    value_str: (*vs).to_string(),
                    name: (*n).to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn parses_simple_trigger_signature() {
        let f = Function::parse(0x0001, "Acquired(S:ResRef*)").unwrap();
        assert_eq!(f.id, 0x0001);
        assert_eq!(f.name, "Acquired");
        assert_eq!(f.params.len(), 1);
        assert_eq!(f.params[0].kind, ParamKind::String);
        assert_eq!(f.params[0].name, "ResRef");
        assert_eq!(f.params[0].ids_ref, "");
    }

    #[test]
    fn parses_signature_with_ids_ref_and_object() {
        let f = Function::parse(0x0002, "AttackedBy(O:Object*,I:Style*AStyles)").unwrap();
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].kind, ParamKind::Object);
        assert_eq!(f.params[1].kind, ParamKind::Integer);
        assert_eq!(f.params[1].ids_ref, "astyles");
    }

    #[test]
    fn parses_zero_arg_function() {
        let f = Function::parse(0, "NoAction()").unwrap();
        assert!(f.params.is_empty());
    }

    #[test]
    fn parses_signature_with_typed_zero_object() {
        // Some IDS files use '0' instead of 'O' for object parameters.
        let f = Function::parse(7, "CreateCreature(0:Target*)").unwrap();
        assert_eq!(f.params.len(), 1);
        assert_eq!(f.params[0].kind, ParamKind::Object);
    }

    #[test]
    fn registers_function_by_id_and_name() {
        let table = Signatures::from_ids(&ids(&[
            (0x0001, "0x0001", "Acquired(S:ResRef*)"),
            (0x4036, "0x4036", "OR(I:OrCount*)"),
        ]));
        assert!(table.get(0x0001).is_some());
        assert!(table.get_by_name("OR").is_some());
        assert!(table.get_by_name("or").is_some()); // case-insensitive
    }
}
