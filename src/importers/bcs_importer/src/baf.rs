//! BCS → BAF decompiler.
//!
//! Mirrors `org.infinity.resource.bcs.Decompiler` from NearInfinity:
//! takes a parsed [`Bcs`] script plus the function-signature tables for
//! TRIGGER.IDS and ACTION.IDS, and emits the human-readable
//! `IF / THEN / RESPONSE / END` source form. Symbol resolution for object
//! identifiers, target specifiers (EA, GENERAL, ...) and integer parameters
//! with an `*IdsRef` is opt-in: the decompiler falls back to the raw
//! number (or `UnknownObject<n>`) whenever the IDS file isn't supplied —
//! matching how NI behaves when running headless without those caches.
//!
//! The decompiler is engine-agnostic: trigger / action names come from the
//! IDS files supplied at runtime and the order of target-specifier IDS
//! resources is provided by the caller, so the same code works for BG, BG2,
//! the EE family, IWD, IWD2 and PST. See [`OBJECT_SPECIFIER_IDS_BG`] and
//! friends for the standard layouts.

use std::collections::HashMap;

use infinitier_common::{Engine, Game};
use infinitier_ids_importer::Ids;

use crate::signatures::{Function, ParamKind, Signatures};
use crate::{Action, Bcs, BcsObject, Trigger};

/// Object specifier slots (target IDS resources in array order) used by BG
/// and BG2 (Enhanced Edition included), Icewind Dale and Icewind Dale EE.
pub(crate) const OBJECT_SPECIFIER_IDS_BG: &[&str] = &[
    "EA", "GENERAL", "RACE", "CLASS", "SPECIFIC", "GENDER", "ALIGN",
];

/// Object specifier slots used by Planescape: Torment (and PSTEE).
pub(crate) const OBJECT_SPECIFIER_IDS_PST: &[&str] = &[
    "EA", "FACTION", "TEAM", "GENERAL", "RACE", "CLASS", "SPECIFIC", "GENDER", "ALIGN",
];

/// Object specifier slots used by Icewind Dale 2.
pub(crate) const OBJECT_SPECIFIER_IDS_IWD2: &[&str] = &[
    "EA", "GENERAL", "RACE", "CLASS", "SPECIFIC", "GENDER", "ALIGNMNT", "SUBRACE", "AVCLASS",
    "CLASSMSK",
];

/// Combined-string encoding for one function id.
///
/// The BCS bytecode has only two string slots per trigger / action, but some
/// functions (Global, SetGlobal, …) actually carry three logical strings —
/// the variable namespace and the variable name are packed into a single
/// slot. This struct describes how to unpack them. Mirrors the bit layout
/// used by NearInfinity's `functionConcatMap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcatInfo {
    /// First bytecode string slot packs two logical strings.
    pub first_combined: bool,
    /// Second bytecode string slot packs two logical strings.
    pub second_combined: bool,
    /// First combined slot uses a `:` separator instead of the fixed 6-char split.
    pub first_colon: bool,
    /// Second combined slot uses a `:` separator instead of the fixed 6-char split.
    pub second_colon: bool,
    /// When non-zero, the entry only applies to functions whose signature has
    /// exactly this number of parameters (NI uses this to disambiguate
    /// signatures sharing an id).
    pub num_params: u16,
}

impl ConcatInfo {
    /// Parses NI's packed `functionConcatMap` value.
    ///
    /// Bit layout: `0xPPPPCCSS`
    /// where `S` = combined flags, `C` = colon flags, `P` = expected parameter count.
    pub const fn from_packed(v: u32) -> Self {
        Self {
            first_combined: (v & 0x0001) != 0,
            second_combined: (v & 0x0010) != 0,
            first_colon: (v & 0x0100) != 0,
            second_colon: (v & 0x1000) != 0,
            num_params: ((v >> 16) & 0xFFFF) as u16,
        }
    }
}

/// Inputs for the BAF decompiler / compiler.
///
/// Build one from your parsed TRIGGER.IDS / ACTION.IDS plus the [`Game`] you
/// are targeting; [`Self::new`] derives every engine-specific knob (object
/// specifier layout, combined-string map, presence of object regions and
/// trigger points, IWD2 trailing target slots) from the game's
/// [`Engine`](infinitier_common::Engine).
///
/// All fields are private so callers can't put the context into an
/// inconsistent state. Optional IDS resources for symbolic resolution
/// (OBJECT.IDS, EA.IDS, …) can be layered in with [`Self::with_ids`]; the
/// indentation string used by the decompiler can be tweaked with
/// [`Self::with_indent`]. Without those, `UnknownObject<n>` and raw numbers
/// are emitted, matching how NearInfinity behaves with its IDS cache empty.
pub struct BafContext {
    triggers: Signatures,
    actions: Signatures,
    /// Names (without extension, upper-case) of the IDS files mapped onto the
    /// object's `targets` array, in slot order. Slot 0 is always EA. Always
    /// one of the engine-specific `OBJECT_SPECIFIER_IDS_*` constants — kept as
    /// a static slice so `BafContext::new` doesn't allocate.
    object_specifier_ids: &'static [&'static str],
    /// IDS files indexed by their upper-cased resource name (without
    /// extension). Used for object identifier nesting (`OBJECT`), target
    /// specifier symbols (`EA`, `GENERAL`, …) and `*IdsRef` integer lookups.
    ids: HashMap<String, Ids>,
    /// Per-id combined-string encoding. Keys are trigger / action ids; the
    /// same id space is shared because the encoding only depends on the
    /// signature's string layout, not on whether the function is a trigger
    /// or an action.
    combined_strings: HashMap<i32, ConcatInfo>,
    /// String prepended once per nesting level. NearInfinity uses `\t` when
    /// running headless, which is what we default to.
    indent: String,
    /// Whether the engine's object bytecode includes the `[x.y.w.h]`
    /// rectangle slot (PST / IWD / IWD2). Used by the BAF compiler to know
    /// whether to write the empty `[-1.-1.-1.-1]` sentinel into the
    /// recompiled bytecode when the BAF has no region.
    object_has_region: bool,
    /// Whether the engine's trigger bytecode includes the `[x,y]` point slot
    /// (PST only).
    trigger_has_point: bool,
    /// Number of object-target slots that the engine emits *after* the name
    /// in OB blocks (IWD2's `T10` / `T11`).
    object_trailing_targets: usize,
}

impl BafContext {
    /// Creates a context for `game`, using its parsed TRIGGER.IDS / ACTION.IDS
    /// signatures. All engine-specific knobs (object specifier layout,
    /// combined-string map, region / point / trailing-target presence) are
    /// derived from `game.engine()`.
    ///
    /// Per-game signature patches are also applied here for functions that
    /// NearInfinity hardcodes because they're missing from the engine's IDS
    /// files — e.g. PST scripts use `Clicked(O:Object*)` (`0x4070`) but the
    /// classic PST install ships a TRIGGER.IDS without that entry. Without
    /// this fix, decompiling PST scripts produces `// Error - Could not find
    /// trigger 0x4070` instead of `Clicked(...)`, and round-trips fail.
    pub fn new(mut triggers: Signatures, actions: Signatures, game: Game) -> Self {
        let engine = game.engine();
        Self::apply_signature_patches(&mut triggers, game);
        let object_specifier_ids: &'static [&'static str] = match engine {
            Engine::Iwd2 => OBJECT_SPECIFIER_IDS_IWD2,
            Engine::Pst => OBJECT_SPECIFIER_IDS_PST,
            // BG, BG2, EE (BGEE / BG2EE / IWDEE / PSTEE / EET) and IWD all
            // use the BG-style 7-slot specifier layout. PSTEE descends from
            // PST but the EE engine reuses the BG bytecode shape, so it
            // belongs here rather than with PST.
            Engine::Bg | Engine::Bg2 | Engine::Ee | Engine::Iwd => OBJECT_SPECIFIER_IDS_BG,
        };
        let combined_strings = match engine {
            Engine::Bg | Engine::Bg2 | Engine::Ee => combined_strings_bg_family(),
            Engine::Iwd => combined_strings_iwd(),
            Engine::Iwd2 => combined_strings_iwd2(),
            Engine::Pst => combined_strings_pst(),
        };
        // PST, IWD and IWD2 reserve a `[x.y.w.h]` slot in OB blocks; the
        // BG family and EE engines (including PSTEE) don't.
        let object_has_region = matches!(engine, Engine::Pst | Engine::Iwd | Engine::Iwd2);
        // Only original PST puts an `[x,y]` point inside trigger blocks.
        let trigger_has_point = matches!(engine, Engine::Pst);
        // IWD2's PARSE_CODE writes two extra target slots after the name.
        let object_trailing_targets = if matches!(engine, Engine::Iwd2) { 2 } else { 0 };

        Self {
            triggers,
            actions,
            object_specifier_ids,
            ids: HashMap::new(),
            combined_strings,
            indent: "\t".to_string(),
            object_has_region,
            trigger_has_point,
            object_trailing_targets,
        }
    }

    /// Applies the per-game signature patches NI's `ScriptInfo` adds at
    /// load time. Today only PST needs this (for `Clicked`); kept as a
    /// dedicated helper so additional NI hardcodings can land here without
    /// reshaping `new`.
    fn apply_signature_patches(triggers: &mut Signatures, game: Game) {
        if matches!(game, Game::Pst) {
            // PST's TRIGGER.IDS is missing `0x4070 Clicked(O:Object*)` even
            // though numerous scripts reference it. Patch it in so the
            // decompiler emits `Clicked(...)` rather than an error comment,
            // and so the compiler resolves it on the way back.
            triggers.add_function(0x4070, "Clicked(O:Object*)");
        }
    }

    /// Registers an IDS resource for symbol resolution. `name` is matched
    /// case-insensitively against the IDS reference (e.g. "OBJECT", "EA").
    pub fn with_ids(mut self, name: &str, ids: Ids) -> Self {
        self.ids.insert(name.to_ascii_uppercase(), ids);
        self
    }

    /// Overrides the per-level indentation string (default: `"\t"`).
    pub fn with_indent(mut self, indent: impl Into<String>) -> Self {
        self.indent = indent.into();
        self
    }

    // Accessors for the sibling `baf_compile` module. Read-only by design —
    // the constructor is the only way to set engine-specific state, so a
    // context can never end up in an inconsistent shape.
    pub(crate) fn triggers(&self) -> &Signatures {
        &self.triggers
    }

    pub(crate) fn actions(&self) -> &Signatures {
        &self.actions
    }

    pub(crate) fn object_specifier_ids(&self) -> &'static [&'static str] {
        self.object_specifier_ids
    }

    pub(crate) fn indent(&self) -> &str {
        &self.indent
    }

    pub(crate) fn object_has_region(&self) -> bool {
        self.object_has_region
    }

    pub(crate) fn trigger_has_point(&self) -> bool {
        self.trigger_has_point
    }

    pub(crate) fn object_trailing_targets(&self) -> usize {
        self.object_trailing_targets
    }

    pub(crate) fn ids_lookup(&self, name: &str) -> Option<&Ids> {
        self.ids.get(&name.to_ascii_uppercase())
    }

    pub(crate) fn concat_info(&self, id: i32, num_params: usize) -> Option<ConcatInfo> {
        self.combined_strings
            .get(&id)
            .copied()
            .filter(|c| c.num_params == 0 || num_params == 0 || c.num_params as usize == num_params)
    }
}

/// Combined-string encodings used by the BG family (BG, BGEE, BG2, BG2EE)
/// and IWDEE.
///
/// IWD's separate engine reassigns several of these ids to BitGlobal /
/// GlobalBitGlobal — use [`combined_strings_iwd`] there. PST and IWD2 use
/// their own maps.
pub(crate) fn combined_strings_bg_family() -> HashMap<i32, ConcatInfo> {
    let entries: &[(i32, u32)] = &[
        // Triggers (stable across BG1/BG2/EE)
        (0x400F, 0x0001), // Global
        (0x4034, 0x0001), // GlobalGT
        (0x4035, 0x0001), // GlobalLT
        // Actions: BG1 base
        (30, 0x0001),  // SetGlobal
        (109, 0x0001), // IncrementGlobal
        (115, 0x0001), // SetGlobalTimer
        // BG2 additions
        (246, 0x0001), // CreateCreatureAtLocation
        (256, 0x0001), // CreateItemGlobal
        (268, 0x0001), // RealSetGlobalTimer
        (297, 0x0001), // MoveToSavedLocation
        (335, 0x0001), // SetTokenGlobal
        // EE additions
        (364, 0x0001), // SetGlobalRandom
        (377, 0x0001), // SetGlobalTimerRandom
    ];
    entries
        .iter()
        .map(|(id, v)| (*id, ConcatInfo::from_packed(*v)))
        .collect()
}

/// Combined-string encodings used by Icewind Dale (the original, non-EE).
///
/// IWD reassigns several BG ids — `0x40A5` is `BitGlobal` (not BG's `Name`),
/// `247` / `248` are `BitGlobal` / `GlobalBitGlobal` actions (not BG's
/// `SetToken` / `SetTokenObject`) — so it needs its own map. Pair with
/// [`OBJECT_SPECIFIER_IDS_BG`].
pub(crate) fn combined_strings_iwd() -> HashMap<i32, ConcatInfo> {
    let entries: &[(i32, u32)] = &[
        // Triggers
        (0x400F, 0x0001), // Global
        (0x4034, 0x0001), // GlobalGT
        (0x4035, 0x0001), // GlobalLT
        (0x40A5, 0x0101), // BitGlobal
        (0x40A6, 0x1111), // GlobalBitGlobal
        // Actions
        (30, 0x0001),  // SetGlobal
        (109, 0x0001), // IncrementGlobal
        (115, 0x0001), // SetGlobalTimer
        (243, 0x0011), // IncrementGlobalOnce
        (247, 0x0101), // BitGlobal
        (248, 0x1111), // GlobalBitGlobal
    ];
    entries
        .iter()
        .map(|(id, v)| (*id, ConcatInfo::from_packed(*v)))
        .collect()
}

/// Combined-string encodings used by Icewind Dale 2.
///
/// Mostly the BG-family layout plus IWD2-specific additions (SpellCastEffect,
/// SetGlobalRandom, SetGlobalTimerOnce, …). Pair with
/// [`OBJECT_SPECIFIER_IDS_IWD2`].
pub(crate) fn combined_strings_iwd2() -> HashMap<i32, ConcatInfo> {
    let entries: &[(i32, u32)] = &[
        (0x400F, 0x0001), // Global
        (0x4034, 0x0001), // GlobalGT
        (0x4035, 0x0001), // GlobalLT
        (30, 0x0001),     // SetGlobal
        (109, 0x0001),    // IncrementGlobal
        (115, 0x0001),    // SetGlobalTimer
        (308, 0x0001),    // SetGlobalTimerOnce
        (243, 0x0011),    // IncrementGlobalOnce
        (0x40A5, 0x0101), // BitGlobal
        (247, 0x0101),    // BitGlobal
        (306, 0x0101),    // SetGlobalRandom
        (307, 0x0101),    // SetGlobalTimerRandom
        (0x40A6, 0x1111), // GlobalBitGlobal
        (289, 0x1010),    // SpellCastEffect
        (248, 0x1111),    // GlobalBitGlobal
    ];
    entries
        .iter()
        .map(|(id, v)| (*id, ConcatInfo::from_packed(*v)))
        .collect()
}

/// Combined-string encodings used by Planescape: Torment (and PSTEE).
///
/// PST has its own large set of bitwise / arithmetic Global functions that
/// pack their string parameters; PSTEE adds the EE-style SetGlobalRandom
/// family on top. The PSTEE-only entries are harmless on PST because the
/// same numeric ids aren't assigned to any function in PST's IDS files.
/// Pair with [`OBJECT_SPECIFIER_IDS_PST`].
pub(crate) fn combined_strings_pst() -> HashMap<i32, ConcatInfo> {
    let entries: &[(i32, u32)] = &[
        // Triggers shared with BG and PST-specific bitwise
        (0x400F, 0x0001), // Global
        (0x4034, 0x0001), // GlobalGT
        (0x4035, 0x0001), // GlobalLT
        (0x407F, 0x0001), // BitCheck
        (0x4080, 0x0001), // GlobalBAND
        (0x4081, 0x0001), // BitCheckExact
        (0x4095, 0x0001), // Xor
        (0x409C, 0x0001), // StuffGlobalRandom
        (0x4109, 0x0001), // StuffGlobalRandom (PSTEE)
        // Two-string-pair triggers (var1+area1, var2+area2 packed each into one slot)
        (0x4082, 0x0011), // GlobalEqualsGlobal
        (0x4083, 0x0011), // GlobalLTGlobal
        (0x4084, 0x0011), // GlobalGTGlobal
        (0x4085, 0x0011), // GlobalANDGlobal
        (0x4086, 0x0011), // GlobalORGlobal
        (0x4087, 0x0011), // GlobalBANDGlobal
        (0x4088, 0x0011), // GlobalBANDGlobalExact
        // Actions
        (30, 0x0001),  // SetGlobal
        (109, 0x0001), // IncrementGlobal
        (115, 0x0001), // SetGlobalTimer
        (227, 0x0001), // GlobalBAND
        (228, 0x0001), // GlobalBOR
        (229, 0x0001), // GlobalSHR
        (230, 0x0001), // GlobalSHL
        (231, 0x0001), // GlobalMAX
        (232, 0x0001), // GlobalMIN
        (244, 0x0001), // BitSet
        (245, 0x0001), // BitClear
        (260, 0x0001), // GlobalXOR
        (364, 0x0001), // SetGlobalRandom (PSTEE)
        (377, 0x0001), // SetGlobalTimerRandom (PSTEE)
        (202, 0x0011), // IncrementGlobalOnce
        (233, 0x0011), // GlobalSetGlobal
        (234, 0x0011), // GlobalAddGlobal
        (235, 0x0011), // GlobalSubGlobal
        (236, 0x0011), // GlobalANDGlobal
        (237, 0x0011), // GlobalORGlobal
        (238, 0x0011), // GlobalBANDGlobal
        (239, 0x0011), // GlobalBORGlobal
        (240, 0x0011), // GlobalSHRGlobal
        (241, 0x0011), // GlobalSHLGlobal
        (242, 0x0011), // GlobalMAXGlobal
        (243, 0x0011), // GlobalMINGlobal
        (261, 0x0011), // GlobalXORGlobal
        // Only the 5-param IncrementGlobalOnce signature combines strings.
        (446, 0x0011 | (5 << 16)), // IncrementGlobalOnce (PSTEE)
    ];
    entries
        .iter()
        .map(|(id, v)| (*id, ConcatInfo::from_packed(*v)))
        .collect()
}

impl Bcs {
    /// Decompiles this script into the BAF source form, mirroring the output
    /// produced by NearInfinity's `Decompiler`.
    ///
    /// Symbolic substitutions only happen for IDS resources registered on the
    /// supplied [`BafContext`]; everything else falls back to the raw integer
    /// form (e.g. object identifiers become `UnknownObject<n>`), matching NI's
    /// behaviour when its IDS cache is empty.
    pub fn to_baf(&self, ctx: &BafContext) -> String {
        let mut out = String::new();
        for cr in &self.condition_responses {
            out.push_str("IF\n");
            decompile_condition(&mut out, &cr.condition.triggers, ctx);
            out.push_str("THEN\n");
            for response in &cr.response_set.responses {
                out.push_str(ctx.indent());
                out.push_str(&format!("RESPONSE #{}\n", response.weight));
                for action in &response.actions {
                    out.push_str(ctx.indent());
                    out.push_str(ctx.indent());
                    out.push_str(&decompile_action(action, ctx));
                    out.push('\n');
                }
            }
            out.push_str("END\n\n");
        }
        out
    }
}

fn decompile_condition(out: &mut String, triggers: &[Trigger], ctx: &BafContext) {
    // OR(N) marks the next N triggers as alternatives — each one is rendered
    // with one extra indent level so the IF block visually groups them.
    let mut or_count: i64 = 0;
    let mut override_pending: Option<&Trigger> = None;
    for trigger in triggers {
        if override_pending.is_none() && is_next_trigger_object(trigger, ctx) {
            override_pending = Some(trigger);
            continue;
        }

        if or_count > 0 {
            out.push_str(ctx.indent());
            // NextTriggerObject markers don't consume an OR slot; only the
            // wrapped triggers count.
            if !is_next_trigger_object(trigger, ctx) {
                or_count -= 1;
            }
        } else if let Some(n) = trigger_or_count(trigger, ctx) {
            or_count = n as i64;
        }

        out.push_str(ctx.indent());
        let text = if let Some(over) = override_pending.take() {
            // The negation belongs to the OUTER TriggerOverride, not to the
            // wrapped inner trigger — mirror NI's `! TriggerOverride(obj, fn(...))`
            // emission.
            let inner_body = decompile_trigger_body(trigger, ctx);
            let obj = decompile_object(&over.target, ctx);
            let mut s = String::new();
            if (trigger.flags & 1) != 0 {
                s.push('!');
            }
            s.push_str(&format!("TriggerOverride({},{})", obj, inner_body));
            s
        } else {
            decompile_trigger(trigger, ctx)
        };
        out.push_str(&text);
        out.push('\n');
    }
    // Edge case: a trailing NextTriggerObject() with no follow-up. Render it
    // raw rather than dropping it, to preserve information.
    if let Some(over) = override_pending {
        if or_count > 0 {
            out.push_str(ctx.indent());
        }
        out.push_str(ctx.indent());
        out.push_str(&decompile_trigger(over, ctx));
        out.push('\n');
    }
}

fn is_next_trigger_object(trigger: &Trigger, ctx: &BafContext) -> bool {
    // Mirrors NI's logic: any signature for this id named `NextTriggerObject`
    // with a single Object parameter qualifies.
    if let Some(funcs) = ctx.triggers().get(trigger.id) {
        funcs.iter().any(|f| {
            f.name.eq_ignore_ascii_case("NextTriggerObject")
                && f.params.len() == 1
                && f.params[0].kind == ParamKind::Object
        })
    } else {
        false
    }
}

fn trigger_or_count(trigger: &Trigger, ctx: &BafContext) -> Option<i32> {
    // OR(N) is detected by name + single-integer signature; the count is t1.
    let funcs = ctx.triggers().get(trigger.id)?;
    let is_or = funcs.iter().any(|f| {
        f.name.eq_ignore_ascii_case("OR")
            && f.params.len() == 1
            && f.params[0].kind == ParamKind::Integer
    });
    if is_or { Some(trigger.t1) } else { None }
}

fn decompile_trigger(trigger: &Trigger, ctx: &BafContext) -> String {
    let body = decompile_trigger_body(trigger, ctx);
    if (trigger.flags & 1) != 0 {
        let mut s = String::with_capacity(body.len() + 1);
        s.push('!');
        s.push_str(&body);
        s
    } else {
        body
    }
}

/// Renders a trigger as `Name(args, ...)` with no leading `!`. The caller
/// decides where to put the negation — for plain triggers it's a prefix,
/// for `TriggerOverride(obj, ...)` wrapping it goes on the outer call.
fn decompile_trigger_body(trigger: &Trigger, ctx: &BafContext) -> String {
    let mut effective_id = trigger.id;
    let funcs = match ctx.triggers().get(effective_id) {
        Some(f) => f,
        None => {
            // NI also tries the id with bit 0x4000 toggled before giving up.
            effective_id ^= 0x4000;
            ctx.triggers().get(effective_id).unwrap_or(&[])
        }
    };
    if funcs.is_empty() {
        return format!("// Error - Could not find trigger 0x{:04X}", trigger.id);
    }
    let function = match best_trigger_match(trigger, funcs) {
        Some(f) => f,
        None => {
            return format!(
                "// Error - Could not find matching signature for trigger 0x{:04X}",
                trigger.id
            );
        }
    };

    let mut sb = String::new();
    sb.push_str(&function.name);
    sb.push('(');
    let concat = ctx.concat_info(function.id, function.params.len());
    let strings = [trigger.t4.as_str(), trigger.t5.as_str()];
    let mut cur_num = 0;
    let mut cur_string = 0;
    let mut cur_obj = 0;
    for (i, p) in function.params.iter().enumerate() {
        if i > 0 {
            sb.push(',');
        }
        match p.kind {
            ParamKind::Integer => {
                let v = trigger_numeric(trigger, cur_num);
                sb.push_str(&decompile_number(v, p, ctx));
                cur_num += 1;
            }
            ParamKind::String => {
                let v = get_string_arg(function, cur_string, strings, concat);
                sb.push('"');
                sb.push_str(v);
                sb.push('"');
                cur_string += 1;
            }
            ParamKind::Object => {
                if cur_obj == 0 {
                    sb.push_str(&decompile_object(&trigger.target, ctx));
                } else {
                    sb.push_str(&decompile_object(&BcsObject::default_empty(), ctx));
                }
                cur_obj += 1;
            }
            ParamKind::Point => {
                let (x, y) = trigger.t7.map(|p| (p.x, p.y)).unwrap_or((0, 0));
                sb.push_str(&format!("[{}.{}]", x, y));
            }
            ParamKind::Action | ParamKind::Trigger => {
                // Not produced in trigger signatures we care about.
            }
        }
    }
    sb.push(')');
    sb
}

fn decompile_action(action: &Action, ctx: &BafContext) -> String {
    let funcs = ctx.actions().get(action.id).unwrap_or(&[]);
    if funcs.is_empty() {
        return format!("// Error - Could not find action {}", action.id);
    }
    let function = match best_action_match(action, funcs) {
        Some(f) => f,
        None => {
            return format!(
                "// Error - Could not find matching signature for action {}",
                action.id
            );
        }
    };

    let mut sb = String::new();
    sb.push_str(&function.name);
    sb.push('(');

    let concat = ctx.concat_info(function.id, function.params.len());
    let strings = [action.a8.as_str(), action.a9.as_str()];
    let mut cur_num = 0;
    let mut cur_string = 0;
    // a1 is reserved for ActionOverride's target — function objects start at a2.
    let mut cur_obj = 1;
    for (i, p) in function.params.iter().enumerate() {
        if i > 0 {
            sb.push(',');
        }
        match p.kind {
            ParamKind::Integer => {
                let v = action_numeric(action, cur_num);
                sb.push_str(&decompile_number(v, p, ctx));
                cur_num += 1;
            }
            ParamKind::String => {
                let v = get_string_arg(function, cur_string, strings, concat);
                sb.push('"');
                sb.push_str(v);
                sb.push('"');
                cur_string += 1;
            }
            ParamKind::Object => {
                let obj = action_object(action, cur_obj);
                sb.push_str(&decompile_object(obj, ctx));
                cur_obj += 1;
            }
            ParamKind::Point => {
                sb.push_str(&format!("[{}.{}]", action.a5_x, action.a5_y));
            }
            ParamKind::Action | ParamKind::Trigger => {}
        }
    }
    sb.push(')');

    // ActionOverride wrapping: when a1 carries a real object, prepend
    // `ActionOverride(<a1>,` and close with an extra `)`. NI also tries to
    // find a custom 2-arg action with id 1 first, falling back to the literal
    // `ActionOverride` name.
    if !is_object_empty(&action.a1) {
        let override_name = ctx
            .actions
            .get(1)
            .and_then(|fs| {
                fs.iter().find(|f| {
                    f.params.len() == 2
                        && f.params[0].kind == ParamKind::Object
                        && f.params[1].kind == ParamKind::Action
                })
            })
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "ActionOverride".to_string());
        let prefix = format!("{}({},", override_name, decompile_object(&action.a1, ctx));
        let mut wrapped = String::with_capacity(prefix.len() + sb.len() + 1);
        wrapped.push_str(&prefix);
        wrapped.push_str(&sb);
        wrapped.push(')');
        sb = wrapped;
    }

    sb
}

fn trigger_numeric(t: &Trigger, idx: usize) -> i32 {
    match idx {
        0 => t.t1,
        1 => t.t2,
        2 => t.t3,
        _ => 0,
    }
}

fn trigger_string(t: &Trigger, idx: usize) -> &str {
    match idx {
        0 => &t.t4,
        1 => &t.t5,
        _ => "",
    }
}

fn action_numeric(a: &Action, idx: usize) -> i32 {
    match idx {
        0 => a.a4,
        1 => a.a6,
        2 => a.a7,
        _ => 0,
    }
}

fn action_string(a: &Action, idx: usize) -> &str {
    match idx {
        0 => &a.a8,
        1 => &a.a9,
        _ => "",
    }
}

/// Returns the logical string argument at `position` for `function`,
/// performing combined-string splitting per `concat`. Mirrors NI's
/// `BcsStructureBase.getStringParam`.
fn get_string_arg<'a>(
    function: &Function,
    position: usize,
    strings: [&'a str; 2],
    concat: Option<ConcatInfo>,
) -> &'a str {
    let mut spos: usize = 0;
    let mut scnt: usize = 0;
    for p in &function.params {
        if p.kind != ParamKind::String {
            continue;
        }
        let (combined, colon) = combined_flags_at(concat, scnt);
        let s = strings.get(spos >> 1).copied().unwrap_or("");
        if scnt == position {
            if combined {
                return split_combined(s, spos & 1 == 0, colon);
            }
            return s;
        }
        spos += if combined { 1 } else { 2 };
        scnt += 1;
    }
    ""
}

fn split_combined(s: &str, even: bool, colon: bool) -> &str {
    let pos = if colon {
        s.find(':').unwrap_or(s.len())
    } else {
        s.len().min(6)
    };
    let ofs = if colon { 1 } else { 0 };
    if even {
        let start = pos.saturating_add(ofs).min(s.len());
        &s[start..]
    } else {
        &s[..pos]
    }
}

/// Computes (combined, colon-separated) flags for the string parameter at
/// `position` (zero-based among string params). Mirrors NI's
/// `ScriptInfo.isCombinedString` / `isColonSeparatedString`.
fn combined_flags_at(concat: Option<ConcatInfo>, position: usize) -> (bool, bool) {
    let Some(c) = concat else {
        return (false, false);
    };
    let mut mask: u32 = (c.first_combined as u32) | ((c.second_combined as u32) << 4);
    let mut mask2: u32 = (c.first_colon as u32) | ((c.second_colon as u32) << 4);
    let mut pos = 0usize;
    while pos < position {
        let ofs = if (mask & 1) != 0 { 2 } else { 1 };
        if position < pos + ofs {
            break;
        }
        pos += ofs;
        mask >>= 4;
        mask2 >>= 4;
    }
    ((mask & 1) != 0, (mask2 & 1) != 0)
}

fn action_object(a: &Action, idx: usize) -> &BcsObject {
    match idx {
        0 => &a.a1,
        1 => &a.a2,
        2 => &a.a3,
        _ => &a.a2,
    }
}

fn is_object_empty(obj: &BcsObject) -> bool {
    obj.targets.iter().all(|&v| v == 0)
        && obj.identifiers.iter().all(|&v| v == 0)
        && obj.name.is_empty()
}

fn decompile_object(object: &BcsObject, ctx: &BafContext) -> String {
    // Mirrors NI's decompileObject:
    //   1. Render the target slot list `[EA.GENERAL.RACE...]` (truncated past
    //      the last non-zero) as the innermost piece, or fall back to the
    //      `name` string if no targets are set.
    //   2. Walk the identifier slots from outer to inner, wrapping the target
    //      with `OuterId(InnerId(target))`.
    //   3. If a search rectangle is set, append `[x.y.w.h]` after everything.
    //   4. If nothing at all was set, output `[ANYONE]`.

    let mut target = decompile_object_target(object, false, ctx);
    if target.is_none() && !object.name.is_empty() {
        target = Some(format!("\"{}\"", object.name));
    }

    let mut identifiers: Option<Vec<String>> = None;
    if object.identifiers.iter().any(|&v| v != 0) {
        let mut list = Vec::new();
        let map = ctx.ids_lookup("OBJECT");
        let mut found = false;
        for i in (0..object.identifiers.len()).rev() {
            let v = object.identifiers[i];
            if v != 0 {
                found = true;
                let symbol = match map.and_then(|m| m.of_value(v)) {
                    Some(s) => normalize_symbol(s).into_owned(),
                    None => format!("UnknownObject{}", v),
                };
                list.push(symbol);
            } else if found {
                break;
            }
        }
        identifiers = Some(list);
    }

    let region_suffix = match &object.region {
        Some(r) if !r.is_empty() => Some(format!("[{}.{}.{}.{}]", r.x, r.y, r.width, r.height)),
        _ => None,
    };

    if target.is_none() && identifiers.is_none() {
        let mut s = "[ANYONE]".to_string();
        if let Some(r) = region_suffix {
            s.push_str(&r);
        }
        return s;
    }

    let mut sb = String::new();
    let mut closing = String::new();
    if let Some(list) = &identifiers {
        let cnt = list.len();
        for (i, sym) in list.iter().enumerate() {
            sb.push_str(sym);
            if i + 1 < cnt || target.is_some() {
                sb.push('(');
                closing.push(')');
            }
        }
    }
    if let Some(t) = &target {
        sb.push_str(t);
    }
    sb.push_str(&closing);
    if let Some(r) = region_suffix {
        sb.push_str(&r);
    }
    sb
}

fn decompile_object_target(
    object: &BcsObject,
    use_default: bool,
    ctx: &BafContext,
) -> Option<String> {
    // Find the highest non-zero target slot; everything past it is implicit zero.
    let mut count = 0;
    for i in (0..object.targets.len()).rev() {
        if object.targets[i] != 0 {
            count = i + 1;
            break;
        }
    }
    if count == 0 {
        return if use_default {
            Some("[ANYONE]".to_string())
        } else {
            None
        };
    }
    let mut sb = String::from("[");
    for i in 0..count {
        if i > 0 {
            sb.push('.');
        }
        let v = object.targets[i];
        let symbol = if v == 0 {
            None
        } else {
            ctx.object_specifier_ids()
                .get(i)
                .copied()
                .and_then(|name| ctx.ids_lookup(name))
                .and_then(|map| map.of_value(v))
                .map(normalize_symbol)
        };
        match symbol {
            Some(s) => sb.push_str(&s),
            None => sb.push_str(&v.to_string()),
        }
    }
    sb.push(']');
    Some(sb)
}

fn decompile_number<'a>(
    value: i32,
    param: &crate::signatures::Parameter,
    ctx: &'a BafContext,
) -> std::borrow::Cow<'a, str> {
    if !param.ids_ref.is_empty()
        && let Some(map) = ctx.ids_lookup(&param.ids_ref)
        && let Some(name) = map.of_value(value)
    {
        return normalize_symbol(name);
    }
    std::borrow::Cow::Owned(value.to_string())
}

/// Returns the symbol untouched when it parses as a script identifier, or
/// wraps it in five quotes (NI's escape for symbols that would otherwise be
/// rejected by the parser). The valid-symbol fast path returns `Cow::Borrowed`
/// to skip an allocation per OBJECT.IDS lookup.
fn normalize_symbol(symbol: &str) -> std::borrow::Cow<'_, str> {
    if is_valid_symbol(symbol) {
        std::borrow::Cow::Borrowed(symbol)
    } else {
        std::borrow::Cow::Owned(format!("\"\"\"\"\"{}\"\"\"\"\"", symbol))
    }
}

fn is_valid_symbol(s: &str) -> bool {
    // Mirrors NI's regex: `[a-zA-Z_][0-9a-zA-Z#_!-]*`
    //                  | `[a-zA-Z#_][a-zA-Z#_!-][0-9a-zA-Z#_!-]*`.
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let alt1 = is_alpha_under(bytes[0]) && bytes[1..].iter().all(|&b| is_id_char(b));
    if alt1 {
        return true;
    }
    bytes.len() >= 2
        && is_alpha_under_hash(bytes[0])
        && is_id_mid_char(bytes[1])
        && bytes[2..].iter().all(|&b| is_id_char(b))
}

fn is_alpha_under(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_alpha_under_hash(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'#'
}

fn is_id_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'#' || b == b'_' || b == b'!' || b == b'-'
}

fn is_id_mid_char(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'#' || b == b'_' || b == b'!' || b == b'-'
}

impl BcsObject {
    fn default_empty() -> Self {
        // Defer to the public `BcsObject::empty()` which fills in the
        // `region` sentinel as well; kept as a thin alias because the
        // decompiler module otherwise wouldn't need to reach for it.
        Self::empty()
    }
}

// -- best-match scoring ------------------------------------------------------
// Picks one signature when an id has multiple candidates (e.g. `7
// CreateCreature` vs `7 CreateCreatureEffect`). The heuristics intentionally
// match NearInfinity's behaviour so the generated text agrees with NI.

fn best_trigger_match<'a>(trigger: &Trigger, funcs: &'a [Function]) -> Option<&'a Function> {
    if funcs.len() == 1 {
        return Some(&funcs[0]);
    }
    let mut sorted: Vec<&Function> = funcs.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    let mut best: Option<&Function> = None;
    let mut best_score_val = i32::MAX;
    let mut best_score_avg = i32::MAX;
    let mut best_num_params = i32::MAX;

    for f in &sorted {
        let mut p_int = 0;
        let mut p_str = 0;
        let mut p_obj = 0;
        let mut p_pt = 0;
        for p in &f.params {
            match p.kind {
                ParamKind::Integer => p_int += 1,
                ParamKind::String => p_str += 1,
                ParamKind::Object => p_obj += 1,
                ParamKind::Point => p_pt += 1,
                _ => {}
            }
        }

        let mut score_int = 0;
        for i in 0..3 {
            if trigger_numeric(trigger, i) != 0 {
                score_int += 1;
            }
        }
        let mut score_str = 0;
        for i in 0..2 {
            if !trigger_string(trigger, i).is_empty() {
                score_str += 1;
            }
        }
        let score_obj = if !is_object_empty(&trigger.target) {
            1
        } else {
            0
        };
        // Triggers in BG/EE/IWD have no point payload, so this is always 0.
        let score_pt = 0;

        let score_int = score_int - p_int;
        let score_str = score_str - p_str;
        let score_obj = score_obj - p_obj;
        let score_pt = score_pt - p_pt;

        let num_params = f.params.len() as i32;
        let score_val = score_int.max(score_str).max(score_obj).max(score_pt);
        let score_avg = (score_int + score_str + score_obj + score_pt).max(0);
        if best.is_none()
            || score_val < best_score_val
            || (score_val == best_score_val
                && (score_avg < best_score_avg
                    || (score_avg == best_score_avg && num_params < best_num_params)))
        {
            best = Some(f);
            best_score_val = score_val;
            best_score_avg = score_avg;
            best_num_params = num_params;
        }
    }
    best
}

fn best_action_match<'a>(action: &Action, funcs: &'a [Function]) -> Option<&'a Function> {
    if funcs.len() == 1 {
        return Some(&funcs[0]);
    }
    let mut sorted: Vec<&Function> = funcs.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    let mut best: Option<&Function> = None;
    let mut fallback: Option<&Function> = None;
    let mut best_score = i32::MAX;
    let mut best_param_count = i32::MAX;

    for f in &sorted {
        let mut pi_count = 0;
        let mut ps_count = 0;
        let mut po_count = 0;
        let mut pp_count = 0;
        for p in &f.params {
            match p.kind {
                ParamKind::Integer => pi_count += 1,
                ParamKind::String => ps_count += 1,
                ParamKind::Object => po_count += 1,
                ParamKind::Point => pp_count += 1,
                _ => {}
            }
        }

        if fallback.is_none() && ps_count > 0 {
            fallback = Some(f);
        }

        let mut pi = 0;
        for i in (0..3).rev() {
            if action_numeric(action, i) != 0 {
                pi = pi_count - i as i32 - 1;
                break;
            }
        }

        // Strings: use first-set-from-end heuristic so that a mostly-empty
        // string slot doesn't disqualify a wider signature.
        let mut ps = 0;
        for i in (0..2).rev() {
            if !action_string(action, i).is_empty() {
                ps = ps_count - i as i32 - 1;
                break;
            }
        }

        let mut po = 0;
        for i in (1..=2).rev() {
            if !is_object_empty(action_object(action, i)) {
                po = po_count - i as i32;
                break;
            }
        }

        let pp = if action.a5_x != 0 || action.a5_y != 0 {
            pp_count - 1
        } else {
            0
        };

        let is_match = pi >= 0 && ps >= 0 && po >= 0 && pp >= 0;
        let param_count = pi_count + ps_count + po_count + pp_count;
        let score = pi + ps + po + pp;
        if is_match && score <= best_score && param_count < best_param_count {
            best_score = score;
            best_param_count = param_count;
            best = Some(f);
        }
    }

    best.or(fallback).or_else(|| sorted.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Bcs, BcsObject, BcsRegion, Condition, ConditionResponse, Response, ResponseSet, Trigger,
    };
    use infinitier_ids_importer::{Ids, IdsEntry};

    fn ids_from(entries: &[(i32, &str)]) -> Ids {
        Ids {
            entries: entries
                .iter()
                .map(|(v, n)| IdsEntry {
                    value: *v,
                    value_str: v.to_string(),
                    name: (*n).to_string(),
                })
                .collect(),
        }
    }

    fn ctx_minimal() -> BafContext {
        let triggers =
            Signatures::from_ids(&ids_from(&[(0x4036, "True()"), (0x4089, "OR(I:OrCount*)")]));
        let actions = Signatures::from_ids(&ids_from(&[(0, "NoAction()")]));
        BafContext::new(triggers, actions, Game::Bg)
    }

    #[test]
    fn empty_object_renders_as_anyone() {
        let obj = BcsObject::empty();
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "[ANYONE]");
    }

    #[test]
    fn object_with_identifiers_uses_unknown_object_fallback() {
        // Identifiers are stored in bytecode order: identifiers[0] is the
        // innermost wrap, the last non-zero slot is the outermost. Without
        // OBJECT.IDS both fall back to UnknownObject.
        let obj = BcsObject {
            identifiers: [1, 12, 0, 0, 0],
            ..BcsObject::empty()
        };
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "UnknownObject12(UnknownObject1)");
    }

    #[test]
    fn object_with_target_renders_as_bracketed_numbers_when_ids_missing() {
        let obj = BcsObject {
            targets: vec![200, 4, 0, 0, 0, 0, 0],
            ..BcsObject::empty()
        };
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "[200.4]");
    }

    #[test]
    fn object_with_loaded_target_ids_resolves_symbols() {
        let obj = BcsObject {
            targets: vec![255, 0, 0, 0, 0, 0, 0],
            ..BcsObject::empty()
        };
        let ctx = ctx_minimal().with_ids("EA", ids_from(&[(255, "ENEMY")]));
        let out = decompile_object(&obj, &ctx);
        assert_eq!(out, "[ENEMY]");
    }

    #[test]
    fn object_with_region_appends_rect() {
        // PST / IWD / IWD2 search rectangles render as `[ANYONE][x.y.w.h]`.
        let obj = BcsObject {
            region: Some(BcsRegion {
                x: 0,
                y: 0,
                width: 10000,
                height: 10000,
            }),
            ..BcsObject::empty()
        };
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "[ANYONE][0.0.10000.10000]");
    }

    #[test]
    fn empty_script_decompiles_to_empty_string() {
        let bcs = Bcs {
            condition_responses: vec![],
        };
        assert_eq!(bcs.to_baf(&ctx_minimal()), "");
    }

    #[test]
    fn or_block_indents_inner_triggers() {
        // Verify OR(2) wraps the next two triggers with one extra indent.
        let triggers =
            Signatures::from_ids(&ids_from(&[(0x4089, "OR(I:OrCount*)"), (0x4036, "True()")]));
        let actions = Signatures::from_ids(&ids_from(&[(0, "NoAction()")]));
        let ctx = BafContext::new(triggers, actions, Game::Bg);

        let bcs = Bcs {
            condition_responses: vec![ConditionResponse {
                condition: Condition {
                    triggers: vec![
                        Trigger {
                            id: 0x4089,
                            flags: 0,
                            t1: 2,
                            t2: 0,
                            t3: 0,
                            t4: String::new(),
                            t5: String::new(),
                            target: BcsObject::empty(),
                            t7: None,
                        },
                        Trigger {
                            id: 0x4036,
                            flags: 0,
                            t1: 0,
                            t2: 0,
                            t3: 0,
                            t4: String::new(),
                            t5: String::new(),
                            target: BcsObject::empty(),
                            t7: None,
                        },
                        Trigger {
                            id: 0x4036,
                            flags: 0,
                            t1: 0,
                            t2: 0,
                            t3: 0,
                            t4: String::new(),
                            t5: String::new(),
                            target: BcsObject::empty(),
                            t7: None,
                        },
                    ],
                },
                response_set: ResponseSet {
                    responses: vec![Response {
                        weight: 100,
                        actions: vec![],
                    }],
                },
            }],
        };

        let out = bcs.to_baf(&ctx);
        let expected = "IF\n\tOR(2)\n\t\tTrue()\n\t\tTrue()\nTHEN\n\tRESPONSE #100\nEND\n\n";
        assert_eq!(out, expected);
    }
}

/// Integration tests that decompile every BCS / BS file in a real game's
/// `extracted_resources/<game>/bcs/original/` and verify byte-exact equality
/// against NearInfinity's reference `bcs/source/<stem>.baf`. Each game has
/// its own `#[test]` so failures stay attributable; tests transparently skip
/// when the game's directory is not present (since the corpus lives outside
/// the repo).
///
/// Override the corpus root via the `EXTRACTED_RESOURCES` env var; it
/// defaults to the path used in this workspace.
#[cfg(test)]
mod corpus_tests {
    use super::*;
    use crate::BcsImporter;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_ids_importer::IdsImporter;
    use std::path::{Path, PathBuf};

    /// Loads `<ids_dir>/<file>` (case-insensitive on extension) into a
    /// `Signatures` table; panics if missing or unparseable.
    fn load_signatures(ids_dir: &Path, file_stem: &str) -> Signatures {
        let path = find_ids(ids_dir, file_stem)
            .unwrap_or_else(|| panic!("missing IDS {}.IDS in {}", file_stem, ids_dir.display()));
        let ids = IdsImporter { name: file_stem }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
        Signatures::from_ids(&ids)
    }

    /// Returns `<ids_dir>/<stem>.IDS`, tolerating the variations used across
    /// game extracts (`TRIGGER.ids`, `TRIGGER.IDS`, `trigger.ids`).
    fn find_ids(ids_dir: &Path, stem: &str) -> Option<PathBuf> {
        for ext in ["IDS", "ids", "Ids"] {
            let p = ids_dir.join(format!("{}.{}", stem, ext));
            if p.is_file() {
                return Some(p);
            }
            let p = ids_dir.join(format!("{}.{}", stem.to_ascii_lowercase(), ext));
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }

    /// Iterates every BCS / BS file in `<corpus_dir>/original`, decompiles it
    /// with `ctx`, and asserts the output matches `<corpus_dir>/source/<stem>.baf`.
    /// Reports a bounded number of failures so a broken decompiler doesn't
    /// produce megabytes of test output.
    fn assert_corpus_matches(corpus_dir: &Path, ctx: &BafContext) {
        let original_dir = corpus_dir.join("original");
        let source_dir = corpus_dir.join("source");
        assert!(original_dir.is_dir(), "missing {}", original_dir.display());
        assert!(source_dir.is_dir(), "missing {}", source_dir.display());

        let mut paths: Vec<PathBuf> = std::fs::read_dir(&original_dir)
            .expect("read original")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| matches!(e.to_ascii_lowercase().as_str(), "bcs" | "bs"))
                    .unwrap_or(false)
            })
            .collect();
        paths.sort();
        assert!(
            !paths.is_empty(),
            "no BCS/BS files in {}",
            original_dir.display()
        );

        let mut failures: Vec<String> = Vec::new();
        const MAX_REPORTED: usize = 5;

        for src_path in &paths {
            let bcs = match (BcsImporter { name: "baf_test" })
                .import(&DataSource::new(src_path.as_path()))
            {
                Ok(b) => b,
                Err(e) => {
                    failures.push(format!("parse error in {}: {}", src_path.display(), e));
                    continue;
                }
            };
            let actual = bcs.to_baf(ctx);
            let stem = src_path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("file stem");
            let baf_path = source_dir.join(format!("{}.baf", stem));
            let expected = match std::fs::read_to_string(&baf_path) {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("missing reference {}: {}", baf_path.display(), e));
                    continue;
                }
            };
            if expected != actual {
                let first_diff = expected
                    .lines()
                    .zip(actual.lines())
                    .enumerate()
                    .find(|(_, (e, a))| e != a)
                    .map(|(i, (e, a))| {
                        format!("  line {}: expected {:?}\n            actual   {:?}", i + 1, e, a)
                    })
                    .unwrap_or_else(|| {
                        format!(
                            "  trailing line difference (expected {} lines / {} bytes, actual {} / {})",
                            expected.lines().count(),
                            expected.len(),
                            actual.lines().count(),
                            actual.len(),
                        )
                    });
                failures.push(format!(
                    "BAF mismatch {}\n{}",
                    baf_path.display(),
                    first_diff
                ));
            }
        }

        if !failures.is_empty() {
            let shown: Vec<String> = failures.iter().take(MAX_REPORTED).cloned().collect();
            panic!(
                "{}/{} files failed (showing first {}):\n{}",
                failures.len(),
                paths.len(),
                shown.len(),
                shown.join("\n\n")
            );
        }
    }

    /// Default corpus root; override with the `EXTRACTED_RESOURCES` env var.
    fn extracted_root() -> PathBuf {
        let raw = std::env::var("EXTRACTED_RESOURCES").unwrap_or_else(|_| {
            "/home/ufo/workspaces/github_ufoscout/baldurs_gate/extracted_resources".to_string()
        });
        PathBuf::from(raw)
    }

    fn build_context(game: Game, ids_dir: &Path) -> BafContext {
        let triggers = load_signatures(ids_dir, "TRIGGER");
        let actions = load_signatures(ids_dir, "ACTION");
        BafContext::new(triggers, actions, game)
    }

    /// Runs the corpus test for `<root>/<dir>/bcs/` against
    /// `<root>/<dir>/ids/`. Skips silently when the game folder is absent.
    fn run_game(dir: &str, game: Game) {
        let root = extracted_root();
        let game_dir = root.join(dir);
        let corpus = game_dir.join("bcs");
        let ids_dir = game_dir.join("ids");
        if !corpus.is_dir() || !ids_dir.is_dir() {
            eprintln!(
                "skip baf corpus test for {}: missing {}",
                dir,
                game_dir.display()
            );
            return;
        }
        let ctx = build_context(game, &ids_dir);
        assert_corpus_matches(&corpus, &ctx);
    }

    #[test]
    fn baf_corpus_bg() {
        run_game("bg", Game::Bg);
    }

    #[test]
    fn baf_corpus_bgee() {
        run_game("bgee", Game::Bgee);
    }

    #[test]
    fn baf_corpus_bg2() {
        run_game("bg2", Game::Bg2);
    }

    #[test]
    fn baf_corpus_bg2ee() {
        run_game("bg2ee", Game::Bg2ee);
    }

    #[test]
    fn baf_corpus_iwd() {
        run_game("iwd", Game::Iwd);
    }

    #[test]
    fn baf_corpus_iwdee() {
        run_game("iwdee", Game::Iwdee);
    }

    #[test]
    fn baf_corpus_iwd2() {
        run_game("iwd2", Game::Iwd2);
    }

    #[test]
    fn baf_corpus_pst() {
        run_game("pst", Game::Pst);
    }

    #[test]
    fn baf_corpus_pstee() {
        run_game("pstee", Game::Pstee);
    }
}
