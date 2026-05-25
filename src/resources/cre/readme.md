# infinitier_cre_resource

Reader and writer for the **CRE** (creature) resource format used in
every Infinity Engine game.

Supports the four on-disk variants documented by IESDP:

| Version | Games                                                  |
|---------|--------------------------------------------------------|
| `V1.0`  | BG, BG2, BG:EE, BG2:EE, IWD:EE, PST:EE, EET            |
| `V1.2`  | PST (vanilla)                                          |
| `V9.0`  | IWD (vanilla + HoW / TotL)                             |
| `V2.2`  | IWD2 (d20 system)                                      |

Each version's on-disk version tag fully determines its layout, so
the importer/exporter takes no `Engine` selector (unlike the GAM
crate, where V1.1 is shared across three engines with different
trailing layouts).

## Parsing scope (Tier-1)

This first cut focuses on **round-trip correctness** and
**structured sub-section access**:

- Signature + version are validated and dispatched.
- The fixed-width header (per version) is preserved as raw bytes
  so the file round-trips losslessly without us having to hand-type
  ~850 distinct field accessors.
- A small per-version "section table" struct surfaces the offsets
  and counts the importer/exporter actually need to navigate the
  file (known-spells, spell-memorisation, memorised-spells, items,
  item-slots, effects, plus IWD2's per-class spell offsets and the
  abilities / songs / shapes blocks).
- Variable-length sub-sections (known spells, memorisation info,
  memorised spells, items, V1 / V2 effects, IWD2-specific records)
  are parsed into structured Rust records.

Field-by-field parsing of the fixed header (stats, AC, HP, saves,
class/race, sounds, scripts, …) is intentionally deferred — it's
purely additive on top of this layer and doesn't change round-trip
behaviour.
