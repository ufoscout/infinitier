# infinitier_itm_resource

Reader and writer for the **ITM** (item) resource format used in
every Infinity Engine game.

Supports the three on-disk variants documented by IESDP:

| Version | Games                                                          | Header |
|---------|---------------------------------------------------------------|--------|
| `V1  `  | BG, BG2, BG:EE, BG2:EE, IWD, IWD:EE, PST:EE, EET              | 114 B  |
| `V1.1`  | PST vanilla                                                    | 154 B  |
| `V2.0`  | IWD2                                                            | 130 B  |

V1.1 extends V1 with the PST-specific Dialog / Conversable-label /
Paperdoll-colour fields plus six "unknown" reserved dwords. V2.0
extends V1 with an unknown 16-byte trailer and tweaks one byte of
the extended-header layout (the four bytes at extended-header
offset 0x26 are a single `Flags` dword in V1 / V1.1 but split into
`Flags` (u16) + `Attack type` (u16) in V2.0 — the byte sequence is
identical, so we expose it as one `flags: u32` and document the
V2 split in the field doc).

Like the SPL / CRE / GAM crates this one **doesn't take an
`Engine` selector** — the on-disk version tag fully determines the
layout. The first byte of the version tag is `'V'`; the byte
sequence at offsets 4..8 is what dispatches the parser.

## Parsed shape

- A typed [`ItmHeader`] enum variant per version, with every
  primitive IESDP field surfaced as a struct field (resrefs as
  `String`, strrefs as `u32`, etc.).
- A list of [`ItmAbility`] records ("extended headers", 56 bytes
  each — same byte size in every version).
- A list of [`ItmEffect`] records (48 bytes each, "feature blocks"
  in IESDP terminology). Each ability references a window of
  effects via `(first_effect_index, num_effects)`; "equipping"
  effects share the same vector but are pointed at by separate
  cursor fields on the header.
