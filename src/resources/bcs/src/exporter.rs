//! BCS bytecode writer.
//!
//! Serialises a [`Bcs`] tree back to the engine's bytecode-text form
//! (`SC/CR/CO/RS/RE/TR/AC/OB` blocks). Round-trips byte-for-byte against
//! every BCS in the test corpus — see the integration test in
//! [`tests::test_all_bcs_files_to_byte_code_round_trip`].
//!
//! Public entry points:
//! - [`BcsExporter`] — the writer-style API used elsewhere in the
//!   workspace (write to a `std::io::Write`, or directly to a file).
//! - [`Bcs::to_byte_code`] — a thin `String`-returning convenience kept
//!   for ergonomic in-memory serialisation (the BAF compiler uses it
//!   internally).

use std::fmt::Write as _;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::{Action, Bcs, BcsObject, ConditionResponse, Response, Trigger};

/// A BCS bytecode exporter.
///
/// Mirrors the writer pattern used by the other resource crates
/// (`BamExporter`, `MosExporter`, …): the unit struct holds no
/// configuration, and the produced bytes round-trip back through
/// [`crate::BcsImporter`] into an equal [`Bcs`].
pub struct BcsExporter;

impl BcsExporter {
    /// Writes `bcs` as BCS bytecode text into `writer`.
    pub fn export<W: Write>(&self, bcs: &Bcs, writer: &mut W) -> io::Result<()> {
        // Build the textual form in a `String` first — BCS files are
        // pure ASCII and small (a few KB per CR block on average), so
        // the extra allocation is cheaper than syscalls per push.
        let text = bcs.to_byte_code();
        writer.write_all(text.as_bytes())
    }

    /// Writes `bcs` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, bcs: &Bcs, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(bcs, &mut writer)?;
        writer.flush()
    }
}

impl Bcs {
    /// Serialises this script to the BCS bytecode-text format (the
    /// `SC/CR/CO/TR/...` encoding stored in game files). Parsing the
    /// returned string through [`crate::BcsImporter`] produces an
    /// equal `Bcs`.
    ///
    /// This is the in-memory convenience; use [`BcsExporter`] when you
    /// want to stream into a `std::io::Write` or write straight to a
    /// file.
    pub fn to_byte_code(&self) -> String {
        // Rough sizing: BCS files in the wild average ~3 KB per CR block. The
        // capacity is just a hint; growth still happens correctly if exceeded.
        let mut out = String::with_capacity(64 + self.condition_responses.len() * 3072);
        out.push_str("SC\n");
        for cr in &self.condition_responses {
            push_condition_response(&mut out, cr);
        }
        out.push_str("SC\n");
        out
    }
}

fn push_condition_response(out: &mut String, cr: &ConditionResponse) {
    out.push_str("CR\n");
    out.push_str("CO\n");
    for trigger in &cr.condition.triggers {
        push_trigger(out, trigger);
    }
    out.push_str("CO\n");
    out.push_str("RS\n");
    for response in &cr.response_set.responses {
        push_response(out, response);
    }
    out.push_str("RS\n");
    out.push_str("CR\n");
}

fn push_trigger(out: &mut String, t: &Trigger) {
    out.push_str("TR\n");
    // PST emits the trigger point `[x,y]` between t3 and t4 (PARSE_CODE_PST =
    // "X1N237456"); other engines (BG / IWD / IWD2) skip it. We rely on the
    // parser populating `t7` only on PST scripts to know whether to write it
    // back here.
    if let Some(p) = t.t7 {
        let _ = writeln!(
            out,
            "{} {} {} {} {} [{},{}] \"{}\" \"{}\" OB",
            t.id, t.t1, t.flags, t.t2, t.t3, p.x, p.y, t.t4, t.t5
        );
    } else {
        let _ = writeln!(
            out,
            "{} {} {} {} {} \"{}\" \"{}\" OB",
            t.id, t.t1, t.flags, t.t2, t.t3, t.t4, t.t5
        );
    }
    push_object_content(out, &t.target);
    out.push_str("TR\n");
}

fn push_response(out: &mut String, r: &Response) {
    out.push_str("RE\n");
    let _ = write!(out, "{}", r.weight);
    for action in &r.actions {
        out.push_str("AC\n");
        push_action(out, action);
    }
    out.push_str("RE\n");
}

fn push_action(out: &mut String, a: &Action) {
    let _ = writeln!(out, "{}OB", a.id);
    push_object_content(out, &a.a1);
    out.push_str("OB\n");
    push_object_content(out, &a.a2);
    out.push_str("OB\n");
    push_object_content(out, &a.a3);
    // no space between a7 and the opening quote — matches the game format
    let _ = writeln!(
        out,
        "{} {} {} {} {}\"{}\" \"{}\" AC",
        a.a4, a.a5_x, a.a5_y, a.a6, a.a7, a.a8, a.a9
    );
}

fn push_object_content(out: &mut String, obj: &BcsObject) {
    // Layout (engine-dependent):
    //   leading-targets  identifiers  [region]  "name"  trailing-targets  OB
    // BG / IWD / PST emit only leading targets (`trailing_targets == 0`).
    // IWD2's parse code interleaves two trailing target slots after the
    // name — those come from `obj.targets` last `trailing_targets` entries.
    let leading_end = obj.targets.len().saturating_sub(obj.trailing_targets);
    for v in &obj.targets[..leading_end] {
        let _ = write!(out, "{} ", v);
    }
    for v in &obj.identifiers {
        let _ = write!(out, "{} ", v);
    }
    if let Some(r) = &obj.region {
        let _ = write!(out, "[{}.{}.{}.{}] ", r.x, r.y, r.width, r.height);
    }
    if obj.trailing_targets > 0 {
        let _ = write!(out, "\"{}\" ", obj.name);
        for (i, v) in obj.targets[leading_end..].iter().enumerate() {
            if i + 1 == obj.trailing_targets {
                let _ = writeln!(out, "{} OB", v);
            } else {
                let _ = write!(out, "{} ", v);
            }
        }
    } else {
        let _ = writeln!(out, "\"{}\"OB", obj.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BcsImporter;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path};

    #[test]
    fn test_all_bcs_files_to_byte_code_round_trip() {
        // For every BCS in the corpus, the writer must reproduce the
        // source file byte-for-byte. This is a strictly stronger check
        // than a parse-then-export equality round-trip: it catches
        // formatting drift (extra whitespace, separator differences)
        // that the parser would happily absorb.
        let bcs_folder = get_assets_path().join("BCS");
        let paths = get_all_in_folder_by_extension(&bcs_folder, "bcs");
        assert!(!paths.is_empty(), "no BCS files found");

        for bcs_path in paths {
            let actual = BcsImporter { name: "bcs_test" }
                .import(&DataSource::new(bcs_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", bcs_path.display()));

            // Convenience method: should match the file verbatim.
            let regenerated = actual.to_byte_code();
            let source = std::fs::read_to_string(&bcs_path).unwrap();
            assert_eq!(
                source,
                regenerated,
                "byte-for-byte mismatch for {}",
                bcs_path.display()
            );

            // Writer-style API: in-memory Vec<u8> + file path must
            // produce the same bytes.
            let mut buf = Vec::new();
            BcsExporter.export(&actual, &mut buf).unwrap();
            assert_eq!(buf, source.as_bytes(), "BcsExporter::export mismatch");

            // …and re-parsing the writer's output yields an equal `Bcs`
            // (catches any future divergence between `to_byte_code` and
            // `BcsExporter::export`).
            let re_imported = BcsImporter { name: "bcs_test" }
                .import(&DataSource::new(buf))
                .unwrap();
            assert_eq!(
                re_imported,
                actual,
                "round-trip Bcs mismatch for {}",
                bcs_path.display()
            );
        }
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let bcs_folder = get_assets_path().join("BCS");
        let paths = get_all_in_folder_by_extension(&bcs_folder, "bcs");
        assert!(!paths.is_empty(), "no BCS files found");
        // Pick the first available BCS — the corpus check above already
        // covers full coverage; this test exists to exercise
        // `export_to_file` specifically (file IO path).
        let src = &paths[0];
        let original = BcsImporter {
            name: "bcs_file_rt",
        }
        .import(&DataSource::new(src.as_path()))
        .unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        BcsExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = BcsImporter {
            name: "bcs_file_rt2",
        }
        .import(&DataSource::new(tmp.path().to_path_buf()))
        .unwrap();
        assert_eq!(re_imported, original);
    }
}
