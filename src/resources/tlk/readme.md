# infinitier_tlk_resource

Full importer + exporter for IE-engine TLK string tables (`dialog.tlk`
/ `dialogF.tlk`). The crate parses the IESDP-documented V1 layout —
header + per-strref entry table + raw strings section — into typed
fields and round-trips every parsed byte:

- `TlkImporter` reads any shipped engine TLK (BG, BG2, EE, IWD, IWD2,
  PST) and exposes the typed `Tlk { version, language_id,
  strings_offset, entries, strings }`. Per-entry sound metadata
  (resref, volume / pitch variance, flag bits) is decoded into typed
  fields rather than left as raw bytes.
- `TlkExporter` writes the `Tlk` back to disk. Round-trip semantics:
  re-importing the exported bytes yields a struct-equal `Tlk`; for
  any source produced by a shipped IE engine the round-trip is also
  **byte-exact**.
- `Tlk::get(strref)` decodes individual strings on demand to avoid
  eagerly allocating one `String` per entry (`dialog.tlk` typically
  has 50–100k entries).

Only TLK V1 is supported — every shipped IE game uses V1. The V2
spec is a community draft that no engine recognises, so the importer
rejects it at signature-validation time.

