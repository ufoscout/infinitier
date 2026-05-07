use std::fmt::Write as _;

use infinitier_datasource::{DataSource, Importer};
use log::debug;
use serde::{Deserialize, Serialize};

pub mod baf;
pub mod baf_compile;
pub mod signatures;

/// A BCS script file importer.
pub struct BcsImporter<'a> {
    pub name: &'a str,
}

impl<'a> Importer for BcsImporter<'a> {
    type T = Bcs;

    fn import(&self, source: &DataSource) -> std::io::Result<Bcs> {
        let mut reader = source.reader()?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        // BCS bytecode is ASCII-only by spec; the parser operates on raw bytes
        // so we skip a `from_utf8_lossy` round-trip (validation + owned copy of
        // the entire file).
        let mut stream = BcsStream::new(&buf);
        let bcs = parse_bcs(&mut stream)?;
        debug!(
            "Loaded {} [BCS]: {} condition-response blocks",
            self.name,
            bcs.condition_responses.len()
        );
        Ok(bcs)
    }
}

/// A parsed BCS script file.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bcs {
    pub condition_responses: Vec<ConditionResponse>,
}

/// One condition–response block (`CR … CR`).
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionResponse {
    pub condition: Condition,
    pub response_set: ResponseSet,
}

/// The condition part (`CO … CO`) — all triggers must be true.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    pub triggers: Vec<Trigger>,
}

/// The response-set part (`RS … RS`) — one response is chosen by weight.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseSet {
    pub responses: Vec<Response>,
}

/// One response (`RE … RE`) with a probability weight and a list of actions.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub weight: i32,
    pub actions: Vec<Action>,
}

/// A trigger (`TR … TR`).
///
/// Parameters follow the BG/BG2 byte-code order: id, t1, flags, t2, t3, t4, t5, target-object.
/// `flags & 1` means the trigger result is negated. `t7` carries the PST-only
/// point parameter; it is `None` outside PST so bytecode round-trips don't
/// emit a phantom `[0,0]` for non-PST scripts.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    pub id: i32,
    pub flags: i32,
    pub t1: i32,
    pub t2: i32,
    pub t3: i32,
    pub t4: String,
    pub t5: String,
    pub target: BcsObject,
    /// PST-only `[x,y]` point parameter; `None` on non-PST scripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t7: Option<BcsPoint>,
}

/// A 2D `(x, y)` point — used for the PST trigger point and as a primitive
/// inside other places where a `[x,y]` literal appears.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BcsPoint {
    pub x: i32,
    pub y: i32,
}

/// An action (`AC … AC`).
///
/// Parameters follow the BG/BG2 byte-code order:
/// id, a1-object, a2-object, a3-object, a4, a5(x,y), a6, a7, a8-string, a9-string.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub id: i32,
    pub a1: BcsObject,
    pub a2: BcsObject,
    pub a3: BcsObject,
    pub a4: i32,
    pub a5_x: i32,
    pub a5_y: i32,
    pub a6: i32,
    pub a7: i32,
    pub a8: String,
    pub a9: String,
}

/// An object parameter (`OB … OB`).
///
/// `targets` carries the engine's target specifier slots (EA, General, Race,
/// Class, …) — 7 entries on BG / BG2 / EE, 9 on PST and 10 on IWD2. The
/// caller pairs each slot with the appropriate IDS file via
/// [`crate::baf::BafContext::object_specifier_ids`]. `identifiers[0..5]` are
/// always the OBJECT.IDS nesting levels regardless of engine.
///
/// `name` is the script name string. `region` carries the rectangular search
/// area used by PST / IWD / IWD2 scripts (`[x.y.w.h]`); on BG it stays at the
/// default empty value.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BcsObject {
    pub targets: Vec<i32>,
    pub identifiers: [i32; 5],
    pub name: String,
    /// Optional `[x.y.w.h]` search rectangle. `None` means the parser saw no
    /// rectangle in the bytecode (BG-family layout); `Some(_)` is preserved
    /// even for the empty sentinel `(-1, -1, -1, -1)` so PST / IWD / IWD2
    /// scripts round-trip back to identical bytecode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<BcsRegion>,
    /// Number of target slots that the bytecode emits *after* the name
    /// (IWD2 only — its `PARSE_CODE` interleaves two extra `T` slots after
    /// `R0:S0`). On every other engine this is `0`.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub trailing_targets: usize,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

/// A `[x.y.w.h]` rectangle attached to a BCS object (PST / IWD / IWD2).
///
/// The all-`-1` value is the canonical "no region" sentinel and is never
/// rendered in the BAF output (matching NI's `isEmptyRect` check).
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct BcsRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for BcsRegion {
    fn default() -> Self {
        Self {
            x: -1,
            y: -1,
            width: -1,
            height: -1,
        }
    }
}

impl BcsRegion {
    /// Returns whether this region is the empty `(-1, -1, -1, -1)` sentinel.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

impl BcsObject {
    /// Returns an object with every slot zeroed and an empty name. Used as
    /// the default trigger target when the BCS bytecode omits the object.
    /// Defaults to the BG-family 7-slot target layout — callers running
    /// against PST or IWD2 corpora will get longer arrays from the parser
    /// directly.
    pub fn empty() -> Self {
        Self {
            targets: vec![0; 7],
            identifiers: [0; 5],
            name: String::new(),
            region: None,
            trailing_targets: 0,
        }
    }
}

// ── Token stream ─────────────────────────────────────────────────────────────

struct BcsStream<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BcsStream<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// Returns the next non-whitespace byte without consuming it.
    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.data.get(self.pos).copied()
    }

    /// If the next non-whitespace content starts with `kw`, consumes those bytes
    /// and returns `true`; otherwise leaves the stream unchanged and returns `false`.
    fn try_skip(&mut self, kw: &str) -> bool {
        self.skip_ws();
        if self.data[self.pos..].starts_with(kw.as_bytes()) {
            self.pos += kw.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kw: &str) -> std::io::Result<()> {
        if self.try_skip(kw) {
            Ok(())
        } else {
            let preview = &self.data[self.pos..self.data.len().min(self.pos + 10)];
            Err(std::io::Error::other(format!(
                "expected '{kw}', got '{}'",
                String::from_utf8_lossy(preview)
            )))
        }
    }

    fn read_i32(&mut self) -> std::io::Result<i32> {
        self.skip_ws();
        let start = self.pos;
        if self.data.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        let num_start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == num_start {
            let preview = &self.data[self.pos..self.data.len().min(self.pos + 10)];
            return Err(std::io::Error::other(format!(
                "expected integer, got '{}'",
                String::from_utf8_lossy(preview)
            )));
        }
        let s = std::str::from_utf8(&self.data[start..self.pos]).unwrap();
        s.parse::<i32>()
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    fn read_string(&mut self) -> std::io::Result<String> {
        self.skip_ws();
        if self.data.get(self.pos) != Some(&b'"') {
            let preview = &self.data[self.pos..self.data.len().min(self.pos + 10)];
            return Err(std::io::Error::other(format!(
                "expected '\"', got '{}'",
                String::from_utf8_lossy(preview)
            )));
        }
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != b'"' {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err(std::io::Error::other("unterminated string literal"));
        }
        // BCS string slots are ASCII in practice; skip the lossy validator's
        // unconditional alloc when the bytes are already valid UTF-8.
        let bytes = &self.data[start..self.pos];
        let s = match std::str::from_utf8(bytes) {
            Ok(s) => s.to_owned(),
            Err(_) => String::from_utf8_lossy(bytes).into_owned(),
        };
        self.pos += 1; // consume closing "
        Ok(s)
    }

    /// Consumes a `[x,y]` point or `[x.y.w.h]` rectangle and returns the raw
    /// integers. Used to skip over PST point parameters and PST/IWD/IWD2
    /// object rectangles, which the public structs don't currently model.
    fn read_point_or_rect(&mut self) -> std::io::Result<Vec<i32>> {
        self.skip_ws();
        if self.data.get(self.pos) != Some(&b'[') {
            return Err(std::io::Error::other("expected '['"));
        }
        self.pos += 1;
        let mut nums = Vec::with_capacity(4);
        loop {
            self.skip_ws();
            // separators between numbers are either '.' or ','; consume one.
            if matches!(self.data.get(self.pos), Some(&b'.') | Some(&b',')) {
                self.pos += 1;
            }
            self.skip_ws();
            if self.data.get(self.pos) == Some(&b']') {
                self.pos += 1;
                return Ok(nums);
            }
            nums.push(self.read_i32()?);
            // Loop continues; either next byte is a separator, ']', or another digit.
        }
    }

    fn is_eos(&self) -> bool {
        self.pos >= self.data.len()
    }
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_bcs(s: &mut BcsStream<'_>) -> std::io::Result<Bcs> {
    // NI's decompiler returns empty output for any file that doesn't start
    // with `SC` (including empty BCS and the misnamed BAF-source files
    // sprinkled through some game extracts, e.g. iwd2/TESTOCLE.bcs). Match
    // that behaviour so the corpus tests don't trip on those.
    if !s.try_skip("SC") {
        return Ok(Bcs {
            condition_responses: Vec::new(),
        });
    }
    let mut condition_responses = Vec::new();
    while !s.is_eos() && !s.try_skip("SC") {
        condition_responses.push(parse_condition_response(s)?);
    }
    Ok(Bcs {
        condition_responses,
    })
}

fn parse_condition_response(s: &mut BcsStream<'_>) -> std::io::Result<ConditionResponse> {
    s.expect("CR")?;
    let condition = parse_condition(s)?;
    let response_set = parse_response_set(s)?;
    s.expect("CR")?;
    Ok(ConditionResponse {
        condition,
        response_set,
    })
}

fn parse_condition(s: &mut BcsStream<'_>) -> std::io::Result<Condition> {
    s.expect("CO")?;
    let mut triggers = Vec::new();
    while !s.is_eos() && !s.try_skip("CO") {
        triggers.push(parse_trigger(s)?);
    }
    Ok(Condition { triggers })
}

fn parse_response_set(s: &mut BcsStream<'_>) -> std::io::Result<ResponseSet> {
    s.expect("RS")?;
    let mut responses = Vec::new();
    while !s.is_eos() && !s.try_skip("RS") {
        responses.push(parse_response(s)?);
    }
    Ok(ResponseSet { responses })
}

fn parse_response(s: &mut BcsStream<'_>) -> std::io::Result<Response> {
    s.expect("RE")?;
    let weight = s.read_i32()?;
    let mut actions = Vec::new();
    while !s.is_eos() && !s.try_skip("RE") {
        actions.push(parse_action(s)?);
    }
    Ok(Response { weight, actions })
}

fn parse_trigger(s: &mut BcsStream<'_>) -> std::io::Result<Trigger> {
    // Triggers are token-driven, not field-positional: BG / BG2 occasionally
    // emit short forms (e.g. `2 0 OB ... TR`) and PST adds a point parameter.
    // We mirror NI's BcsTrigger.init: read whatever comes next until we hit
    // the closing TR, dispatching by lookahead.
    s.expect("TR")?;
    let mut nums = [0i32; 5];
    let mut num_count = 0;
    let mut strings: [String; 2] = [String::new(), String::new()];
    let mut str_count = 0;
    let mut target: Option<BcsObject> = None;
    let mut t7: Option<BcsPoint> = None;
    loop {
        if s.try_skip("TR") {
            break;
        }
        if s.try_skip("OB") {
            // OB is consumed; parse the rest of the object body and store as
            // the (single) trigger target slot.
            let obj = parse_object_body(s)?;
            if target.is_none() {
                target = Some(obj);
            }
            continue;
        }
        match s.peek() {
            Some(b'-') | Some(b'0'..=b'9') => {
                let n = s.read_i32()?;
                if num_count < nums.len() {
                    nums[num_count] = n;
                }
                num_count += 1;
            }
            Some(b'"') => {
                let st = s.read_string()?;
                if str_count < strings.len() {
                    strings[str_count] = st;
                }
                str_count += 1;
            }
            Some(b'[') => {
                // PST trigger point parameter `[x,y]`.
                let parts = s.read_point_or_rect()?;
                if parts.len() >= 2 {
                    t7 = Some(BcsPoint {
                        x: parts[0],
                        y: parts[1],
                    });
                }
            }
            None => return Err(std::io::Error::other("unexpected EOS inside TR")),
            Some(c) => {
                return Err(std::io::Error::other(format!(
                    "unexpected token in TR: {:?}",
                    c as char
                )));
            }
        }
    }
    Ok(Trigger {
        id: nums[0],
        t1: nums[1],
        flags: nums[2],
        t2: nums[3],
        t3: nums[4],
        t4: std::mem::take(&mut strings[0]),
        t5: std::mem::take(&mut strings[1]),
        target: target.unwrap_or_else(BcsObject::empty),
        t7,
    })
}

fn parse_action(s: &mut BcsStream<'_>) -> std::io::Result<Action> {
    // Like parse_trigger, action bytecode is token-driven: BG1's older scripts
    // sometimes omit the trailing string parameters entirely (`... 0 0 0 0
    // 85AC` with no `""` slots), so we scan tokens until the closing AC.
    s.expect("AC")?;
    let mut nums = [0i32; 6];
    let mut num_count = 0;
    let mut strings: [String; 2] = [String::new(), String::new()];
    let mut str_count = 0;
    let mut objects: Vec<BcsObject> = Vec::with_capacity(3);
    loop {
        if s.try_skip("AC") {
            break;
        }
        if s.try_skip("OB") {
            objects.push(parse_object_body(s)?);
            continue;
        }
        match s.peek() {
            Some(b'-') | Some(b'0'..=b'9') => {
                let n = s.read_i32()?;
                if num_count < nums.len() {
                    nums[num_count] = n;
                }
                num_count += 1;
            }
            Some(b'"') => {
                let st = s.read_string()?;
                if str_count < strings.len() {
                    strings[str_count] = st;
                }
                str_count += 1;
            }
            None => return Err(std::io::Error::other("unexpected EOS inside AC")),
            Some(c) => {
                return Err(std::io::Error::other(format!(
                    "unexpected token in AC: {:?}",
                    c as char
                )));
            }
        }
    }

    let mut take_obj = |i: usize| -> BcsObject {
        if i < objects.len() {
            std::mem::replace(&mut objects[i], BcsObject::empty())
        } else {
            BcsObject::empty()
        }
    };

    Ok(Action {
        id: nums[0],
        a4: nums[1],
        a5_x: nums[2],
        a5_y: nums[3],
        a6: nums[4],
        a7: nums[5],
        a1: take_obj(0),
        a2: take_obj(1),
        a3: take_obj(2),
        a8: std::mem::take(&mut strings[0]),
        a9: std::mem::take(&mut strings[1]),
    })
}

/// Parses everything between an already-consumed opening `OB` and its
/// closing `OB`. Mirrors NI's BcsObject.init: token-driven, with the
/// 5 OBJECT.IDS identifiers carved out of the position immediately before
/// the rectangle (or the name, if no rectangle is present). On BG / BG2 /
/// EE this matches "last 5 numbers"; on IWD2 the layout interleaves the
/// rectangle and additional target slots after the identifiers, which is
/// why we have to track positions explicitly.
fn parse_object_body(s: &mut BcsStream<'_>) -> std::io::Result<BcsObject> {
    // Single-pass scan: stash integer values with their token positions so we
    // can carve out the 5 identifier slots once we've seen the separator
    // (rect preferred, name otherwise — matching NI's `pos_rect.or(pos_name)`
    // resolution).
    let mut int_pos: Vec<usize> = Vec::with_capacity(16);
    let mut int_val: Vec<i32> = Vec::with_capacity(16);
    let mut pos_rect: Option<usize> = None;
    let mut pos_name: Option<usize> = None;
    let mut name = String::new();
    let mut region: Option<BcsRegion> = None;
    let mut tok_pos: usize = 0;
    loop {
        if s.try_skip("OB") {
            break;
        }
        match s.peek() {
            Some(b'-') | Some(b'0'..=b'9') => {
                int_pos.push(tok_pos);
                int_val.push(s.read_i32()?);
            }
            Some(b'"') => {
                let st = s.read_string()?;
                pos_name.get_or_insert(tok_pos);
                name = st;
            }
            Some(b'[') => {
                let parts = s.read_point_or_rect()?;
                let r = if parts.len() == 4 {
                    [parts[0], parts[1], parts[2], parts[3]]
                } else {
                    [-1, -1, -1, -1]
                };
                pos_rect.get_or_insert(tok_pos);
                region = Some(BcsRegion {
                    x: r[0],
                    y: r[1],
                    width: r[2],
                    height: r[3],
                });
            }
            None => return Err(std::io::Error::other("unexpected EOS inside OB")),
            Some(c) => {
                return Err(std::io::Error::other(format!(
                    "unexpected token in OB: {:?}",
                    c as char
                )));
            }
        }
        tok_pos += 1;
    }

    let pos_separator = pos_rect.or(pos_name).unwrap_or(tok_pos);
    let id_start = pos_separator.saturating_sub(5);
    let id_end = id_start + 5;
    let post_name_start = pos_name.map(|p| p + 1).unwrap_or(tok_pos);

    let mut targets: Vec<i32> = Vec::with_capacity(int_val.len());
    let mut identifiers = [0i32; 5];
    let mut ident_idx = 0;
    let mut trailing_targets = 0usize;
    for (i, &v) in int_pos.iter().zip(int_val.iter()) {
        if *i >= id_start && *i < id_end {
            if ident_idx < 5 {
                identifiers[ident_idx] = v;
                ident_idx += 1;
            }
        } else {
            targets.push(v);
            if *i >= post_name_start {
                trailing_targets += 1;
            }
        }
    }

    Ok(BcsObject {
        targets,
        identifiers,
        name,
        region,
        trailing_targets,
    })
}

impl Bcs {
    /// Serializes this script to the BCS byte-code text format (the SC/CR/CO/TR/… encoding
    /// stored in game files). Parsing the returned string produces an equal `Bcs`.
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
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path, parse_json_file};

    fn parse_object(s: &mut BcsStream<'_>) -> std::io::Result<BcsObject> {
        s.expect("OB")?;
        parse_object_body(s)
    }

    #[test]
    fn test_parse_empty_script() {
        let mut s = BcsStream::new(b"SC\nSC\n");
        let bcs = parse_bcs(&mut s).unwrap();
        assert_eq!(bcs.condition_responses.len(), 0);
    }

    #[test]
    fn test_parse_simple_object_all_zeros() {
        let src = "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n";
        let mut s = BcsStream::new(src.as_bytes());
        let obj = parse_object(&mut s).unwrap();
        assert_eq!(obj.targets, vec![0; 7]);
        assert_eq!(obj.identifiers, [0; 5]);
        assert_eq!(obj.name, "");
    }

    #[test]
    fn test_parse_object_with_ea_and_identifiers() {
        // ea=30 in targets, identifiers=[1,12,0,0,0]
        let src = "OB\n30 0 0 0 0 0 0 1 12 0 0 0 \"\"OB\n";
        let mut s = BcsStream::new(src.as_bytes());
        let obj = parse_object(&mut s).unwrap();
        assert_eq!(obj.targets, vec![30, 0, 0, 0, 0, 0, 0]);
        assert_eq!(obj.identifiers, [1, 12, 0, 0, 0]);
        assert_eq!(obj.name, "");
    }

    #[test]
    fn test_parse_object_with_name() {
        let src = "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"Caveentrance\"OB\n";
        let mut s = BcsStream::new(src.as_bytes());
        let obj = parse_object(&mut s).unwrap();
        assert_eq!(obj.name, "Caveentrance");
    }

    #[test]
    fn test_parse_trigger() {
        let src = "TR\n47 111 0 0 0 \"\" \"\" OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\nTR\n";
        let mut s = BcsStream::new(src.as_bytes());
        let t = parse_trigger(&mut s).unwrap();
        assert_eq!(t.id, 47);
        assert_eq!(t.t1, 111);
        assert_eq!(t.flags, 0);
        assert_eq!(t.t2, 0);
        assert_eq!(t.t3, 0);
        assert_eq!(t.t4, "");
        assert_eq!(t.t5, "");
    }

    #[test]
    fn test_parse_trigger_negated() {
        let src = "TR\n16395 255 1 0 0 \"\" \"\" OB\n0 0 0 0 0 0 0 1 0 0 0 0 \"\"OB\nTR\n";
        let mut s = BcsStream::new(src.as_bytes());
        let t = parse_trigger(&mut s).unwrap();
        assert_eq!(t.id, 16395);
        assert_eq!(t.t1, 255);
        assert_eq!(t.flags, 1);
        assert_eq!(t.target.identifiers, [1, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_trigger_with_string_param() {
        let src = "TR\n16399 1 0 0 0 \"GLOBALReturnedOutside\" \"\" OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\nTR\n";
        let mut s = BcsStream::new(src.as_bytes());
        let t = parse_trigger(&mut s).unwrap();
        assert_eq!(t.t4, "GLOBALReturnedOutside");
        assert_eq!(t.t5, "");
    }

    #[test]
    fn test_parse_action() {
        let src = concat!(
            "AC\n",
            "22OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n",
            "OB\n0 0 0 0 0 0 0 1 12 0 0 0 \"\"OB\n",
            "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n",
            "0 0 0 0 0\"\" \"\" AC\n"
        );
        let mut s = BcsStream::new(src.as_bytes());
        let a = parse_action(&mut s).unwrap();
        assert_eq!(a.id, 22);
        assert_eq!(a.a2.identifiers, [1, 12, 0, 0, 0]);
        assert_eq!(a.a4, 0);
        assert_eq!(a.a5_x, 0);
        assert_eq!(a.a5_y, 0);
        assert_eq!(a.a8, "");
        assert_eq!(a.a9, "");
    }

    #[test]
    fn test_parse_action_with_params() {
        let src = concat!(
            "AC\n",
            "30OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n",
            "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n",
            "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n",
            "2 0 0 0 0\"GLOBALReturnedOutside\" \"\" AC\n"
        );
        let mut s = BcsStream::new(src.as_bytes());
        let a = parse_action(&mut s).unwrap();
        assert_eq!(a.id, 30);
        assert_eq!(a.a4, 2);
        assert_eq!(a.a8, "GLOBALReturnedOutside");
        assert_eq!(a.a9, "");
    }

    #[test]
    fn test_all_bcs_files() {
        let bcs_folder = get_assets_path().join("resources/BCS");
        let paths = get_all_in_folder_by_extension(&bcs_folder, "bcs");
        assert!(!paths.is_empty(), "no BCS files found");

        for bcs_path in paths {
            let actual = BcsImporter { name: "bcs_test" }
                .import(&DataSource::new(bcs_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", bcs_path.display()));

            // Test parsing the BCS file
            {
                let json_path = bcs_path.with_extension("json");
                let expected: Bcs = parse_json_file(&json_path);
                assert_eq!(actual, expected, "BCS mismatch for {}", bcs_path.display());
            }

            // Test that BCS `to_byte_code` reproduces the original BCS file
            {
                let bcs_bytes_generated = actual.to_byte_code();
                let bcs_from_bytes = BcsImporter { name: "bcs_test" }
                    .import(&DataSource::new(bcs_bytes_generated.as_bytes()))
                    .unwrap_or_else(|e| panic!("cannot import {}: {e}", bcs_path.display()));
                assert_eq!(
                    bcs_from_bytes,
                    actual,
                    "BCS mismatch for {}",
                    bcs_path.display()
                );

                let bcs_from_file: String = std::fs::read_to_string(bcs_path).unwrap();
                assert_eq!(bcs_from_file, bcs_bytes_generated);
            }
        }
    }
}
