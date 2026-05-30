# Keeper: resource-driven data audit

Audit of values that today are hardcoded in the keeper and which of
them *could* (or *should*) come from the loaded game resources
(`*.2DA`, `*.IDS`, `dialog.tlk`) instead.

The audit was taken after the `EngineCaps` refactor that moved the
AD&D ability bonus tables (`STRMOD.2DA`, `STRMODEX.2DA`,
`DEXMOD.2DA`, `HPCONBON.2DA`) from hardcoded match-expressions to
runtime 2DA reads via `BonusTable::from_two_da`. The same pattern is
the obvious template for everything in the "could be 2DA-driven" list
below.

## Status legend

- **🟢 Worth doing** — visible improvement, low/medium implementation
  cost, fits the existing `EngineCaps`-style "load at startup, cache,
  consult per-row" pattern.
- **🟡 When the relevant tab lands** — the keeper doesn't surface the
  data today; wire the lookup the day the tab is implemented.
- **🔴 Don't touch** — the value is engine-binary-hardcoded, format-
  defined, or English-only UI; reading it from a resource adds
  indirection without benefit.

---

## 🟢 2DA-driven, worth implementing

### 1. `HPWAR.2DA` — warrior HP-per-level bonus

**What:** Vanilla AD&D 2e gives warrior classes (Fighter / Ranger /
Paladin) larger HP bonuses from CON than other classes. Today
`EngineCaps::constitution_hp_bonus` reads `HPCONBON.2DA` (the
non-warrior table) for everyone, with a comment in
`components/editable_fields.rs` noting the warrior shortcut.

**Source:** `HPWAR.2DA`, same shape as `HPCONBON.2DA` (`HP_BONUS`
column keyed by CON score). Class detection comes from the CRE's
`class` byte (V10/V12/V90) cross-referenced against `CLASS.IDS`.

**Surface:** The "(effective: Max HP)" indicator on the Combat &
status card. Today it under-reports for warriors (e.g. a CON 18
fighter gets +4 HP/level vanilla but the keeper shows +2).

**Blockers:**

- Need class detection helper on `cre_fields` / `components`. Today
  the keeper has no notion of "which class is this".
- Need to load `CLASS.IDS` (or hardcode the small set of warrior
  class IDs — `FIGHTER`, `RANGER`, `PALADIN`, and in EE multi/dual
  combinations involving them).

**Estimated effort:** Medium. Class detection is a stepping stone
for items 2/3/4 below.

---

### 2. `XPCAP.2DA` — class-specific XP caps

**What:** Today `EngineCaps.experience: AbilityRange<u32>` is
`u32::MIN..=u32::MAX` (storage-only). Vanilla games cap class XP per
table — e.g. BG2 fighters cap at 2 950 000.

**Source:** `XPCAP.2DA` (column = class, row = some discriminator);
exact layout varies per engine (BG1/BG2/EE all slightly different).
Combined with the same class detection as item 1.

**Surface:** Clamp the Experience input on the Experience & levels
card so the user can't accidentally enter a value the engine would
refuse to honor at level-up.

---

### 3. `SAVEXXX.2DA` family — saving throws by class and level

**What:** `SAVEFIG.2DA`, `SAVEMAGE.2DA`, `SAVECLR.2DA`, `SAVEPRS.2DA`,
`SAVERNG.2DA`, `SAVEWAR.2DA`, `SAVEMONK.2DA`, … one per class, columns
are the five save categories (Death, Wand, Polymorph, Breath, Spell),
rows indexed by level.

**Surface:** Not displayed today. The day a "Saves" tab is wired up,
each row's max effective save = base table value − any item / spell
modifiers. The base value comes from the appropriate `SAVE*.2DA`.

**Status:** Wait until the tab lands. Loading + caching the per-class
table is the same `BonusTable`-shaped pattern as `STRMOD` / `DEXMOD`,
just keyed by `(class, level)` instead of by score.

---

### 4. `THIEF.2DA` / `THIEFSCL.2DA` — class-based thief-skill caps

**What:** Today the keeper exposes raw thief-skill bytes
(`hide_in_shadows_base`, `move_silently`, `lockpicking`, …) clamped
to `u8::MAX`. The actual class-progression maxima come from
`THIEFSCL.2DA` (per-class allocation per level) and the level-up UI
in-game uses those to decide how many points to give.

**Surface:** Could either (a) clamp the input against the class /
level limit, or (b) just show the cap as a tooltip. Either way it
needs class + level detection (items 1 + 2 already give us this).

---

### 5. `CLSRCREQ.2DA`, `ABCLASRQ.2DA` — class / race minimum ability scores

**What:** Per-class minimum ability scores (e.g. paladin needs STR
12, CHA 17 in AD&D 2e). Stored as a 6-column 2DA, rows = class.

**Surface:** Could refuse to clamp STR below the class minimum, or
flag a warning on the card. Marginal benefit; mostly a sanity-check
for fictional-stat scenarios.

---

### 6. `COLOR.2DA` (and `CLOWNRGE` / `CLOWNCOL` variants) — appearance palette

**What:** Per-engine palette that the in-game appearance UI uses to
populate the four colour pickers (Skin, Hair, Major, Minor). Each
row is an indexed palette ramp.

**Surface:** The Appearance tab is a stub today (`ui/tabs/appearance.rs`
is the "Not implemented yet" placeholder). When it grows colour
pickers, this 2DA drives them.

---

## 🟡 IDS-driven, when the relevant tab lands

These resources map small integer IDs to human-readable names. They
matter the day the keeper displays the corresponding field — not
before.

### 7. `CLASS.IDS` — class names

Used by items 1, 2, 3, 4. Also needed if a "Class: Fighter" line
ever appears on the character header.

### 8. `KIT.IDS` — kit names

EE-engine kits (Berserker, Cavalier, Kensai, Bounty Hunter, …).
Keyed by the kit field on the CRE V12 / V90 header.

### 9. `RACE.IDS` — race names

Human / Elf / Dwarf / Halfling / Half-Elf / Half-Orc / Gnome. Keyed
by the CRE's race byte. Trivial table; could even be hardcoded but
the IDS file is the canonical source per engine.

### 10. `ALIGN.IDS` — alignments

`LAWFUL_GOOD`, `NEUTRAL_GOOD`, … keyed by the CRE's alignment byte.

### 11. `STATE.IDS` — persistent status flags

Bit positions for the "permanent status flags" `u32` on every CRE
header. Used the day an "Effects" tab grows.

### 12. `SLOTS.IDS` — inventory slot names

Per-engine slot layout (V10 puts helmet in slot 0, V90 V22 differ).
Drives the Inventory tab's slot labels.

### 13. `ANIMATE.IDS` — animation IDs

`PARAVAL_M`, `BOY_HUMAN`, … one ID per character sprite. Drives a
"set sprite" dropdown if the appearance tab gains one.

### 14. `SOUNDOFF.IDS` — voice set names

Used by the in-game "select voice" dropdown. Optional appearance-tab
addition.

### 15. BCS / DLG-only: `TRIGGER.IDS`, `ACTION.IDS`, `OBJECT.IDS`

Required only if a script/dialog editor lands. Out of scope for the
abilities/combat/inventory keeper.

---

## 🔴 Stays hardcoded — engine-binary or format-defined

These look hardcoded but **don't appear in any 2DA**. They're either
engine C-code constraints or file-format definitions baked into the
binary that ships in the game's `.exe`.

### Engine-binary caps (in `EngineCaps`)

| Field                  | Cap                        | Why hardcoded                                                                                  |
|------------------------|----------------------------|------------------------------------------------------------------------------------------------|
| `ability_score`        | `1..=25` (AD&D), `1..=30` (IWD2) | Refused by the engine's character creation / level-up code, not stored anywhere as data. |
| `reputation`           | `0..=20`                   | Hardcoded clamp in the engine load / write paths.                                              |
| `morale`               | `0..=20`                   | Hardcoded clamp.                                                                               |
| `morale_break`         | `0..=20`                   | Hardcoded clamp.                                                                               |
| `attacks_byte`         | `0..=10`                   | The `NumberOfAttacks` enum mapping is engine-internal: bytes 0..=5 = literal count, 6..=10 = halves 0.5/1.5/2.5/3.5/4.5. |
| `strength_percentile`  | `0..=100`                  | File-format definition (one byte, treated as percentile by the engine).                        |
| `current_hit_points`   | `u16::MIN..=u16::MAX`      | Storage-type cap.                                                                              |
| `max_hit_points`       | `u16::MIN..=u16::MAX`      | Storage-type cap (XP-cap-style clamping would come from `XPCAP.2DA`-equivalents — see item 2).|
| `armor_class`          | `i16::MIN..=i16::MAX`      | Storage-type cap.                                                                              |
| `thac0`                | `i8::MIN..=i8::MAX`        | Storage-type cap.                                                                              |
| `party_gold`           | `u32::MIN..=u32::MAX`      | Storage-type cap.                                                                              |
| `fatigue/intoxication/luck` | `u8` full range         | Storage-type cap, engine treats high bytes as "very high" with no clamp.                       |
| `experience / xp_for_kill` | `u32` full range       | Storage-type cap; class XP cap is `XPCAP.2DA` — see item 2.                                    |
| `class_level`          | `u8` full range            | Storage-type cap; class progression is `XPLEVEL.2DA`.                                          |
| `morale_recovery`      | `u16` full range           | Storage-type cap.                                                                              |
| `thief_skill / lore`   | `u8` full range            | Storage-type cap; class caps from `THIEFSCL.2DA` — see item 4.                                 |

### CRE / GAM header field definitions

| Item                                            | Why hardcoded                                                                                  |
|-------------------------------------------------|------------------------------------------------------------------------------------------------|
| Per-version field-name ↔ byte-offset mappings in `infinitier_cre_resource::header_generated` | IESDP-defined CRE file format. The Rust struct *is* the format specification. |
| V22 d20 skill field names (`alchemy`, `bluff`, `animal_empathy`, …) | Defined by the CRE V2.2 file format; the engine's parser has these baked in. |
| IWD2 class level field names (`barbarian_levels`, `cleric_levels`, …) | Same — V2.2 field set, IESDP-defined.                                          |
| GAM engine-specific data (`BgGamData`, `Bg2GamData`, …)            | Per-engine GAM layouts, IESDP-defined.                                         |

### UI strings — English-only by design

| Item                                                  | Why hardcoded                                                                                  |
|-------------------------------------------------------|------------------------------------------------------------------------------------------------|
| Row labels: "Strength", "AC (natural)", "Max HP", …   | Keeper UI is English. The corresponding STRREF + 2DA chain (`STATS.2DA`, TLK) adds indirection that hurts more than it helps. Revisit if/when the keeper localizes. |
| Section card titles: "Ability scores", "Combat & status", … | Same. UI copy belongs in the keeper, not in the game data.                                |
| `AttacksOption` labels ("0", "0.5", "1", "1.5", …)    | These mirror the `NumberOfAttacks` enum byte mapping — engine-internal, not 2DA-driven.       |

### Constants in `components/party_selector.rs`

| Constant                       | Why hardcoded                                                                                |
|--------------------------------|----------------------------------------------------------------------------------------------|
| `PORTRAIT_FALLBACK_ASPECT`     | Used only when no portrait is resolved yet. The actual displayed slot uses the *loaded* portrait's dimensions, so this is a one-frame fallback. Could be per-engine but the marginal value is tiny. |
| `PORTRAIT_MAX_HEIGHT`          | Safety cap so a freak portrait can't push the slider below the viewport. Pure UI policy.    |
| `RAIL_WIDTH`                   | Initial sidebar width. UI policy.                                                            |

### Constants in `ui/header_panel.rs`

| Constant            | Why hardcoded                                              |
|---------------------|------------------------------------------------------------|
| `FIELD_MAX_WIDTH`   | Per-column width cap for the top metadata strip. UI policy.|
| `FIELD_MIN_RENDER_WIDTH` | Below this, skip painting the column. UI policy.      |

### Game-detection paths in `infinitier_core::game_detect`

Per-engine sentinel files (`EET.flag`, `data/eetTU00.bif`, etc.).
These are the *evidence* the keeper uses to identify the engine
before any 2DA is even reachable, so by definition they can't come
from a 2DA.

---

## Implementation roadmap (suggested order)

The shortest path that maximizes user-visible value:

1. **Class detection helper** — adds `class(cre: &Cre) -> u8` and
   `is_warrior_class(class: u8) -> bool` to `components/cre_fields.rs`.
   The "is warrior" predicate can hardcode the small set of warrior
   class IDs initially and graduate to a `CLASS.IDS` lookup later.
2. **`HPWAR.2DA` load** — extend `EngineCaps::new` with one more
   `BonusTable` (`hpwar_hp`). Add `EngineCaps::constitution_hp_bonus_for_class(class, con)` that
   dispatches between `hpwar_hp` and `hpconbon_hp`. Update the
   abilities tab's "(effective Max HP)" computation to use it.
3. **`XPCAP.2DA` load** — class-specific XP clamping on the
   Experience field. Requires class detection from step 1.
4. **Saves tab** (new) — `SAVEXXX.2DA` family loads. Same shape as
   `BonusTable` but keyed by `(class, level)`. New tab module in
   `ui/tabs/saves.rs`.
5. **Class / race / kit display** — `CLASS.IDS`, `RACE.IDS`,
   `KIT.IDS` loaders. Tiny IDS files; cache once. Display on the
   character header.
6. **Inventory tab** (new) — `SLOTS.IDS` for slot labels;
   `ITM` resource imports for tooltips.
7. **Appearance tab** (new) — `COLOR.2DA`, `ANIMATE.IDS`,
   `SOUNDOFF.IDS` for the four colour pickers + voice/sprite
   dropdowns.

Steps 1–3 share a single short refactor (class detection helpers)
and visibly improve the abilities tab in the most common edit
case — warriors leveling up.

---

## Testing posture

The `BonusTable` test pattern in [`engine_caps.rs`](../src/core/src/engine_caps.rs)
hand-builds `TwoDA` structs in-memory so the unit tests don't need
any game install fixture. Future 2DA loaders should follow the same
pattern: keep the lookup logic on the table struct (`BonusTable`,
`SavingThrowTable`, `XpCapTable`, …) testable in isolation, and gate
the "load from `GameData`" step on a real game install via an
optional integration test pointed at `infinitier_test_utils`-managed
fixtures.

If we ever want a regression guard that "vanilla `STRMOD` still
gives +1 THAC0 for STR 18", the cleanest path is committing a small
set of canonical 2DA fixtures into `assets/KEY/bg2/` (or a new
`assets/2DA/bg2_vanilla/` folder) and adding integration tests that
load them through `BonusTable::from_two_da_any`.
