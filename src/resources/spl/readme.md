# infinitier_spl_resource

Reader and writer for the **SPL** (spell) resource format used in
every Infinity Engine game.

Supports the two on-disk variants documented by IESDP:

| Version | Games                                                  | Header |
|---------|--------------------------------------------------------|--------|
| `V1  `  | BG, BG2, BG:EE, BG2:EE, IWD, IWD:EE, PST, PST:EE, EET  | 114 B  |
| `V2.0`  | IWD2                                                    | 130 B  |

V2.0 is a strict superset of V1: same 114 bytes, plus two
`duration_modifier_*` bytes at `0x72`/`0x73` and a 14-byte trailer
the engine still uses as unknown padding.

Like the CRE / GAM crates this one **doesn't take an `Engine`
selector** — the on-disk version tag fully determines the layout.

## Parsed shape

- A typed [`SplHeader`] enum variant per version, with every
  primitive field documented by IESDP surfaced as a struct field
  (resrefs as `String`, strrefs as `u32`, etc.).
- A list of [`SplAbility`] records (40 bytes each, also called
  "extended headers" by IESDP — one per scaling band of the spell).
- A flat list of [`SplEffect`] records (48 bytes each, IESDP's
  "feature blocks"). Each ability references a window of effects
  via `(first_effect_index, num_effects)`; "casting" feature blocks
  share the same vector but are pointed at by a separate cursor
  on the header.
