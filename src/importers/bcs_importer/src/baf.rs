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

use infinitier_ids_importer::Ids;

use crate::signatures::{Function, ParamKind, Signatures};
use crate::{Action, Bcs, BcsObject, Trigger};

/// Object specifier slots (target IDS resources in array order) used by BG
/// and BG2 (Enhanced Edition included), Icewind Dale and Icewind Dale EE.
pub const OBJECT_SPECIFIER_IDS_BG: &[&str] =
    &["EA", "GENERAL", "RACE", "CLASS", "SPECIFIC", "GENDER", "ALIGN"];

/// Object specifier slots used by Planescape: Torment (and PSTEE).
pub const OBJECT_SPECIFIER_IDS_PST: &[&str] = &[
    "EA", "FACTION", "TEAM", "GENERAL", "RACE", "CLASS", "SPECIFIC", "GENDER", "ALIGN",
];

/// Object specifier slots used by Icewind Dale 2.
pub const OBJECT_SPECIFIER_IDS_IWD2: &[&str] = &[
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

/// Inputs for the BAF decompiler.
///
/// Build one from your parsed TRIGGER.IDS / ACTION.IDS plus the engine's
/// object-specifier layout. Optional IDS resources can be added with
/// [`BafContext::with_ids`] to enable symbolic substitutions for object
/// identifiers, target specifiers and integer parameters that carry an
/// `*IdsRef`. Anything not supplied falls back to the raw numeric form.
pub struct BafContext {
    /// Trigger function signatures (parsed from TRIGGER.IDS).
    pub triggers: Signatures,
    /// Action function signatures (parsed from ACTION.IDS).
    pub actions: Signatures,
    /// Names (without extension, upper-case) of the IDS files mapped onto the
    /// object's `targets` array, in slot order. Slot 0 is always EA.
    pub object_specifier_ids: Vec<String>,
    /// IDS files indexed by their upper-cased resource name (without
    /// extension). Used for object identifier nesting (`OBJECT`), target
    /// specifier symbols (`EA`, `GENERAL`, …) and `*IdsRef` integer lookups.
    pub ids: HashMap<String, Ids>,
    /// Per-id combined-string encoding. Keys are trigger / action ids; the
    /// same id space is shared because the encoding only depends on the
    /// signature's string layout, not on whether the function is a trigger
    /// or an action. Use [`combined_strings_bg_family`] for the standard
    /// BG/BG2/IWD/EE Global-style packings; supply your own map for other
    /// engines.
    pub combined_strings: HashMap<i32, ConcatInfo>,
    /// String prepended once per nesting level. NearInfinity uses `\t` when
    /// running headless, which is what we default to.
    pub indent: String,
}

impl BafContext {
    /// Creates a context for BG/BG2/EE/IWD/IWDEE games (BG-style object
    /// specifiers, tab indent, BG-family combined-string map, no IDS lookups
    /// beyond TRIGGER/ACTION).
    pub fn new_bg(triggers: Signatures, actions: Signatures) -> Self {
        Self {
            triggers,
            actions,
            object_specifier_ids: OBJECT_SPECIFIER_IDS_BG.iter().map(|s| s.to_string()).collect(),
            ids: HashMap::new(),
            combined_strings: combined_strings_bg_family(),
            indent: "\t".to_string(),
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

    /// Replaces the combined-string map. Pass an empty map to disable
    /// combined-string handling entirely.
    pub fn with_combined_strings(mut self, map: HashMap<i32, ConcatInfo>) -> Self {
        self.combined_strings = map;
        self
    }

    fn ids_lookup(&self, name: &str) -> Option<&Ids> {
        self.ids.get(&name.to_ascii_uppercase())
    }

    fn concat_info(&self, id: i32, num_params: usize) -> Option<ConcatInfo> {
        self.combined_strings.get(&id).copied().filter(|c| {
            c.num_params == 0 || num_params == 0 || c.num_params as usize == num_params
        })
    }
}

/// Combined-string encodings shared by the BG, BG2, EE (BGEE/BG2EE), IWD and
/// IWDEE engines. Each entry packs its first (and sometimes second) string
/// parameter into a single bytecode slot.
///
/// The id values are stable across that engine family — `Global` is always
/// `0x400F`, `SetGlobal` is always `30`, etc. — so the same map works for
/// all of them. PST, PSTEE and IWD2 layer additional entries; build a custom
/// map for those.
pub fn combined_strings_bg_family() -> HashMap<i32, ConcatInfo> {
    // Sourced from NearInfinity's ScriptInfo for BG1/BG2/EE/IWD profiles.
    let entries: &[(i32, u32)] = &[
        // Triggers
        (0x400F, 0x0001), // Global
        (0x4034, 0x0001), // GlobalGT
        (0x4035, 0x0001), // GlobalLT
        (0x40A5, 0x0101), // BitGlobal (IWD)
        (0x40A6, 0x1111), // GlobalBitGlobal (IWD)
        // Actions
        (30, 0x0001),  // SetGlobal
        (109, 0x0001), // IncrementGlobal
        (115, 0x0001), // SetGlobalTimer
        (243, 0x0011), // IncrementGlobalOnce (IWD)
        (247, 0x0101), // BitGlobal (IWD)
        (248, 0x1111), // GlobalBitGlobal (IWD)
        (246, 0x0001), // CreateCreatureAtLocation (BG2)
        (256, 0x0001), // CreateItemGlobal (BG2)
        (268, 0x0001), // RealSetGlobalTimer (BG2)
        (297, 0x0001), // MoveToSavedLocation (BG2)
        (335, 0x0001), // SetTokenGlobal (BG2)
        (364, 0x0001), // SetGlobalRandom (EE)
        (377, 0x0001), // SetGlobalTimerRandom (EE)
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
                out.push_str(&ctx.indent);
                out.push_str(&format!("RESPONSE #{}\n", response.weight));
                for action in &response.actions {
                    out.push_str(&ctx.indent);
                    out.push_str(&ctx.indent);
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
            out.push_str(&ctx.indent);
            // NextTriggerObject markers don't consume an OR slot; only the
            // wrapped triggers count.
            if !is_next_trigger_object(trigger, ctx) {
                or_count -= 1;
            }
        } else if let Some(n) = trigger_or_count(trigger, ctx) {
            or_count = n as i64;
        }

        out.push_str(&ctx.indent);
        let text = if let Some(over) = override_pending.take() {
            let inner = decompile_trigger(trigger, ctx);
            let obj = decompile_object(&over.target, ctx);
            format!("TriggerOverride({},{})", obj, inner)
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
            out.push_str(&ctx.indent);
        }
        out.push_str(&ctx.indent);
        out.push_str(&decompile_trigger(over, ctx));
        out.push('\n');
    }
}

fn is_next_trigger_object(trigger: &Trigger, ctx: &BafContext) -> bool {
    // Mirrors NI's logic: any signature for this id named `NextTriggerObject`
    // with a single Object parameter qualifies.
    if let Some(funcs) = ctx.triggers.get(trigger.id) {
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
    let funcs = ctx.triggers.get(trigger.id)?;
    let is_or = funcs.iter().any(|f| {
        f.name.eq_ignore_ascii_case("OR")
            && f.params.len() == 1
            && f.params[0].kind == ParamKind::Integer
    });
    if is_or { Some(trigger.t1) } else { None }
}

fn decompile_trigger(trigger: &Trigger, ctx: &BafContext) -> String {
    let mut effective_id = trigger.id;
    let funcs = match ctx.triggers.get(effective_id) {
        Some(f) => f,
        None => {
            // NI also tries the id with bit 0x4000 toggled before giving up.
            effective_id ^= 0x4000;
            ctx.triggers.get(effective_id).unwrap_or(&[])
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
    if (trigger.flags & 1) != 0 {
        sb.push('!');
    }
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
                // Triggers in BG/EE/IWD don't carry a point; emit a default.
                sb.push_str("[0.0]");
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
    let funcs = ctx.actions.get(action.id).unwrap_or(&[]);
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

fn trigger_string<'a>(t: &'a Trigger, idx: usize) -> &'a str {
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

fn action_string<'a>(a: &'a Action, idx: usize) -> &'a str {
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
    let Some(c) = concat else { return (false, false); };
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

fn action_object<'a>(a: &'a Action, idx: usize) -> &'a BcsObject {
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
    //   3. If nothing at all was set, output `[ANYONE]`.

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
                let symbol = map
                    .and_then(|m| m.of_value(v))
                    .map(normalize_symbol)
                    .unwrap_or_else(|| format!("UnknownObject{}", v));
                list.push(symbol);
            } else if found {
                break;
            }
        }
        identifiers = Some(list);
    }

    if target.is_none() && identifiers.is_none() {
        return "[ANYONE]".to_string();
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
            ctx.object_specifier_ids
                .get(i)
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

fn decompile_number(value: i32, param: &crate::signatures::Parameter, ctx: &BafContext) -> String {
    if !param.ids_ref.is_empty()
        && let Some(map) = ctx.ids_lookup(&param.ids_ref)
        && let Some(name) = map.of_value(value)
    {
        return normalize_symbol(name);
    }
    value.to_string()
}

/// Returns the symbol untouched when it parses as a script identifier, or
/// wraps it in five quotes (NI's escape for symbols that would otherwise be
/// rejected by the parser).
fn normalize_symbol(symbol: &str) -> String {
    if is_valid_symbol(symbol) {
        symbol.to_string()
    } else {
        format!("\"\"\"\"\"{}\"\"\"\"\"", symbol)
    }
}

fn is_valid_symbol(s: &str) -> bool {
    // Mirrors NI's regex: `[a-zA-Z_][0-9a-zA-Z#_!-]*`
    //                  | `[a-zA-Z#_][a-zA-Z#_!-][0-9a-zA-Z#_!-]*`.
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let alt1 = is_alpha_under(bytes[0])
        && bytes[1..].iter().all(|&b| is_id_char(b));
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
        Self {
            targets: [0; 7],
            identifiers: [0; 5],
            name: String::new(),
        }
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
        let score_obj = if !is_object_empty(&trigger.target) { 1 } else { 0 };
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
    use crate::{Bcs, BcsObject, Condition, ConditionResponse, Response, ResponseSet, Trigger};
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
        let triggers = Signatures::from_ids(&ids_from(&[
            (0x4036, "True()"),
            (0x4089, "OR(I:OrCount*)"),
        ]));
        let actions = Signatures::from_ids(&ids_from(&[(0, "NoAction()")]));
        BafContext::new_bg(triggers, actions)
    }

    #[test]
    fn empty_object_renders_as_anyone() {
        let obj = BcsObject {
            targets: [0; 7],
            identifiers: [0; 5],
            name: String::new(),
        };
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "[ANYONE]");
    }

    #[test]
    fn object_with_identifiers_uses_unknown_object_fallback() {
        // Identifiers are stored in bytecode order: identifiers[0] is the
        // innermost wrap, the last non-zero slot is the outermost. Without
        // OBJECT.IDS both fall back to UnknownObject.
        let obj = BcsObject {
            targets: [0; 7],
            identifiers: [1, 12, 0, 0, 0],
            name: String::new(),
        };
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "UnknownObject12(UnknownObject1)");
    }

    #[test]
    fn object_with_target_renders_as_bracketed_numbers_when_ids_missing() {
        let mut targets = [0i32; 7];
        targets[0] = 200;
        targets[1] = 4;
        let obj = BcsObject {
            targets,
            identifiers: [0; 5],
            name: String::new(),
        };
        let out = decompile_object(&obj, &ctx_minimal());
        assert_eq!(out, "[200.4]");
    }

    #[test]
    fn object_with_loaded_target_ids_resolves_symbols() {
        let mut targets = [0i32; 7];
        targets[0] = 255;
        let obj = BcsObject {
            targets,
            identifiers: [0; 5],
            name: String::new(),
        };
        let ctx = ctx_minimal().with_ids("EA", ids_from(&[(255, "ENEMY")]));
        let out = decompile_object(&obj, &ctx);
        assert_eq!(out, "[ENEMY]");
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
        let triggers = Signatures::from_ids(&ids_from(&[
            (0x4089, "OR(I:OrCount*)"),
            (0x4036, "True()"),
        ]));
        let actions = Signatures::from_ids(&ids_from(&[(0, "NoAction()")]));
        let ctx = BafContext::new_bg(triggers, actions);

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
                            target: BcsObject {
                                targets: [0; 7],
                                identifiers: [0; 5],
                                name: String::new(),
                            },
                        },
                        Trigger {
                            id: 0x4036,
                            flags: 0,
                            t1: 0,
                            t2: 0,
                            t3: 0,
                            t4: String::new(),
                            t5: String::new(),
                            target: BcsObject {
                                targets: [0; 7],
                                identifiers: [0; 5],
                                name: String::new(),
                            },
                        },
                        Trigger {
                            id: 0x4036,
                            flags: 0,
                            t1: 0,
                            t2: 0,
                            t3: 0,
                            t4: String::new(),
                            t5: String::new(),
                            target: BcsObject {
                                targets: [0; 7],
                                identifiers: [0; 5],
                                name: String::new(),
                            },
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
