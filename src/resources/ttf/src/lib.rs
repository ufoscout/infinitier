#![doc = include_str!("../readme.md")]

use std::io::Read;
use std::sync::Arc;

use infinitier_datasource::{DataSource, Importer};
use log::{debug, warn};

/// A TrueType / OpenType font importer.
///
/// Parses the `name`, `head`, and `hhea` tables for the metadata an
/// in-app viewer wants to display (family / subfamily / version /
/// designer / em metrics / glyph count) and keeps a shared reference to
/// the raw font bytes so the viewer can install the font into its own
/// text renderer for sample-text rendering without re-reading the file.
pub struct TtfImporter<'a> {
    pub name: &'a str,
}

impl Importer for TtfImporter<'_> {
    type T = Ttf;

    fn import(&self, source: &DataSource) -> std::io::Result<Ttf> {
        let mut reader = source.reader()?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        let metadata = parse_metadata(&bytes).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to parse TTF '{}': {e}", self.name),
            )
        })?;
        debug!(
            "Loaded {} [TTF]: family='{}' subfamily='{}' glyphs={}",
            self.name, metadata.family_name, metadata.subfamily_name, metadata.glyph_count
        );
        Ok(Ttf {
            raw: Arc::new(bytes),
            family_name: metadata.family_name,
            subfamily_name: metadata.subfamily_name,
            full_name: metadata.full_name,
            postscript_name: metadata.postscript_name,
            version: metadata.version,
            copyright: metadata.copyright,
            designer: metadata.designer,
            manufacturer: metadata.manufacturer,
            units_per_em: metadata.units_per_em,
            ascender: metadata.ascender,
            descender: metadata.descender,
            line_gap: metadata.line_gap,
            glyph_count: metadata.glyph_count,
            is_monospaced: metadata.is_monospaced,
        })
    }
}

/// A decoded TTF/OTF font.
#[derive(Clone)]
pub struct Ttf {
    /// Original font bytes, shared so the viewer (or any other
    /// consumer) can install them into a text renderer (e.g. egui's
    /// `FontData::from_owned`) without a second read of the file.
    pub raw: Arc<Vec<u8>>,
    /// Name ID 1 — the font family ("Lato", "Eadui", …).
    pub family_name: String,
    /// Name ID 2 — sub-family / style ("Regular", "Bold Italic", …).
    pub subfamily_name: String,
    /// Name ID 4 — the typeface's display name as the foundry expects
    /// it ("Lato Bold").
    pub full_name: String,
    /// Name ID 6 — PostScript name. Useful for some embedding scenarios.
    pub postscript_name: Option<String>,
    /// Name ID 5 — version string ("Version 1.104; …").
    pub version: Option<String>,
    /// Name ID 0 — copyright notice, if present.
    pub copyright: Option<String>,
    /// Name ID 9 — typeface designer's name, if present.
    pub designer: Option<String>,
    /// Name ID 8 — manufacturer / foundry name, if present.
    pub manufacturer: Option<String>,
    /// Units per em from the `head` table — coordinate space for all
    /// glyph outlines.
    pub units_per_em: u16,
    /// Maximum height above the baseline (typographic ascent).
    pub ascender: i16,
    /// Maximum depth below the baseline (typographic descent — usually
    /// negative).
    pub descender: i16,
    /// Recommended gap between lines.
    pub line_gap: i16,
    /// Number of glyphs in the font (from `maxp`).
    pub glyph_count: u16,
    /// `true` when every glyph has the same advance width.
    pub is_monospaced: bool,
}

impl std::fmt::Debug for Ttf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ttf")
            .field("family_name", &self.family_name)
            .field("subfamily_name", &self.subfamily_name)
            .field("full_name", &self.full_name)
            .field("postscript_name", &self.postscript_name)
            .field("version", &self.version)
            .field("copyright", &self.copyright)
            .field("designer", &self.designer)
            .field("manufacturer", &self.manufacturer)
            .field("units_per_em", &self.units_per_em)
            .field("ascender", &self.ascender)
            .field("descender", &self.descender)
            .field("line_gap", &self.line_gap)
            .field("glyph_count", &self.glyph_count)
            .field("is_monospaced", &self.is_monospaced)
            .field("raw.len()", &self.raw.len())
            .finish()
    }
}

/// Internal parsing result — kept separate from the public struct so
/// the import path can build `Ttf` in one shot.
struct Metadata {
    family_name: String,
    subfamily_name: String,
    full_name: String,
    postscript_name: Option<String>,
    version: Option<String>,
    copyright: Option<String>,
    designer: Option<String>,
    manufacturer: Option<String>,
    units_per_em: u16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
    glyph_count: u16,
    is_monospaced: bool,
}

fn parse_metadata(bytes: &[u8]) -> Result<Metadata, ttf_parser::FaceParsingError> {
    let face = ttf_parser::Face::parse(bytes, 0)?;

    // OpenType `name` table IDs we care about; see the OT spec §6.4.
    // We prefer English (lang_id == 0x409) when multiple translations
    // exist, falling back to the first decodable name otherwise.
    let mut family_name = None;
    let mut subfamily_name = None;
    let mut full_name = None;
    let mut postscript_name = None;
    let mut version = None;
    let mut copyright = None;
    let mut designer = None;
    let mut manufacturer = None;

    for record in face.names() {
        let Some(decoded) = record.to_string() else {
            // Unsupported encoding — skip rather than fail the import.
            continue;
        };
        let prefer = record.language_id == 0x409;
        let slot: &mut Option<String> = match record.name_id {
            0 => &mut copyright,
            1 => &mut family_name,
            2 => &mut subfamily_name,
            4 => &mut full_name,
            5 => &mut version,
            6 => &mut postscript_name,
            8 => &mut manufacturer,
            9 => &mut designer,
            _ => continue,
        };
        if slot.is_none() || prefer {
            *slot = Some(decoded);
        }
    }

    if family_name.is_none() {
        warn!("TTF has no name ID 1 (family); using \"Unknown\"");
    }

    Ok(Metadata {
        family_name: family_name.unwrap_or_else(|| "Unknown".to_string()),
        subfamily_name: subfamily_name.unwrap_or_else(|| "Regular".to_string()),
        full_name: full_name.clone().unwrap_or_else(|| "Unknown".to_string()),
        postscript_name,
        version,
        copyright,
        designer,
        manufacturer,
        units_per_em: face.units_per_em(),
        ascender: face.ascender(),
        descender: face.descender(),
        line_gap: face.line_gap(),
        glyph_count: face.number_of_glyphs(),
        is_monospaced: face.is_monospaced(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_test_utils::get_assets_path;

    #[test]
    fn test_parse_lato_bold() {
        let data = DataSource::new(get_assets_path().join("TTF/Lato-Bold.ttf"));
        let ttf = TtfImporter { name: "lato-bold" }.import(&data).unwrap();
        assert_eq!(ttf.family_name, "Lato");
        // Subfamily is sometimes "Bold", sometimes "Regular" with a
        // separate weight axis — just check it's non-empty for
        // forward-compat. The full name is the foundry's authoritative
        // display string.
        assert!(!ttf.subfamily_name.is_empty());
        assert!(ttf.full_name.contains("Lato"));
        assert!(ttf.glyph_count > 0);
        assert!(ttf.units_per_em > 0);
        assert!(ttf.ascender > 0);
        // Lato is a proportional font.
        assert!(!ttf.is_monospaced);
    }

    #[test]
    fn test_raw_bytes_match_source() {
        // The `raw` field must hold the exact file contents — the
        // viewer relies on this to install the font into egui's text
        // renderer without re-reading from disk.
        let path = get_assets_path().join("TTF/Lato-Bold.ttf");
        let expected = std::fs::read(&path).unwrap();
        let ttf = TtfImporter { name: "lato-bold" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(ttf.raw.as_slice(), expected.as_slice());
    }

    #[test]
    fn test_rejects_non_ttf_bytes() {
        // Random garbage must surface as InvalidData, not panic.
        let data = DataSource::new(&b"this is not a font"[..]);
        let err = TtfImporter { name: "garbage" }.import(&data).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
