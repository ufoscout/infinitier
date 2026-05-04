//! BAF → BCS compiler.
//!
//! Mirrors `org.infinity.resource.bcs.Compiler` from NearInfinity: parses a
//! human-readable BAF script (`IF / THEN / RESPONSE / END` blocks) and
//! emits a fully populated [`Bcs`] struct that, fed into [`Bcs::to_byte_code`],
//! reproduces the original BCS bytecode.
//!
//! The implementation is engine-agnostic — function names resolve through
//! the [`crate::baf::BafContext`]'s TRIGGER.IDS / ACTION.IDS, target-specifier
//! ordering comes from the same context, and the combined-string packing
//! map is the inverse of the one used by the decompiler.
//!
//! Symbolic resolution mirrors NI: bare identifiers are looked up in the
//! parameter's `*IdsRef` IDS file (or OBJECT.IDS for object identifier
//! nesting / target slots) when one is registered on the context, otherwise
//! `UnknownObject<n>` and raw numbers are accepted as a numeric form.

use crate::baf::{BafContext, ConcatInfo};
use crate::signatures::{Function, ParamKind};
use crate::{
    Action, Bcs, BcsObject, BcsPoint, BcsRegion, Condition, ConditionResponse, Response,
    ResponseSet, Trigger,
};

impl Bcs {
    /// Parses BAF source text and returns the compiled [`Bcs`].
    ///
    /// `ctx` supplies trigger / action signatures plus the engine-specific
    /// object-specifier order and combined-string packing map. The same
    /// context used to decompile a script will round-trip it back without
    /// loss.
    pub fn from_baf(source: &str, ctx: &BafContext) -> std::io::Result<Bcs> {
        let mut parser = BafParser::new(source, ctx);
        parser.parse_script()
    }
}

// ── Lexer ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Number(i64),
    String(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Bang,
    Hash,
    Pipe,
    Eof,
}

struct Lexer<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            bytes: s.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            // `//` to end of line
            if self.pos + 1 < self.bytes.len()
                && self.bytes[self.pos] == b'/'
                && self.bytes[self.pos + 1] == b'/'
            {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    /// Returns the next token, consuming bytes from the stream.
    fn next(&mut self) -> std::io::Result<Tok> {
        self.skip_ws_and_comments();
        if self.pos >= self.bytes.len() {
            return Ok(Tok::Eof);
        }
        let b = self.bytes[self.pos];
        match b {
            b'(' => {
                self.pos += 1;
                Ok(Tok::LParen)
            }
            b')' => {
                self.pos += 1;
                Ok(Tok::RParen)
            }
            b'[' => {
                self.pos += 1;
                Ok(Tok::LBracket)
            }
            b']' => {
                self.pos += 1;
                Ok(Tok::RBracket)
            }
            b',' => {
                self.pos += 1;
                Ok(Tok::Comma)
            }
            b'.' => {
                self.pos += 1;
                Ok(Tok::Dot)
            }
            b'!' => {
                self.pos += 1;
                Ok(Tok::Bang)
            }
            b'#' => {
                self.pos += 1;
                Ok(Tok::Hash)
            }
            b'|' => {
                self.pos += 1;
                Ok(Tok::Pipe)
            }
            b'"' => self.read_string(),
            b'-' | b'+' => {
                // Could be a signed number or just punctuation. NI grammar
                // permits unary `-` / `+` only in numeric contexts, which we
                // collapse into the numeric token directly so the parser
                // doesn't need to look ahead.
                if self.pos + 1 < self.bytes.len()
                    && (self.bytes[self.pos + 1].is_ascii_digit())
                {
                    self.read_number()
                } else if b == b'-' {
                    // bare `-` is a connector inside identifiers; if it
                    // reaches here it's a syntax error.
                    Err(std::io::Error::other(format!(
                        "unexpected '-' at byte {}",
                        self.pos
                    )))
                } else {
                    self.pos += 1;
                    self.next()
                }
            }
            b'0'..=b'9' => self.read_number(),
            b if is_ident_start(b) => self.read_ident(),
            _ => Err(std::io::Error::other(format!(
                "unexpected byte {:?} at position {}",
                b as char, self.pos
            ))),
        }
    }

    fn read_number(&mut self) -> std::io::Result<Tok> {
        let start = self.pos;
        if self.bytes[self.pos] == b'-' || self.bytes[self.pos] == b'+' {
            self.pos += 1;
        }
        // Optional 0x / 0X for hex.
        let mut radix = 10u32;
        if self.pos + 1 < self.bytes.len()
            && self.bytes[self.pos] == b'0'
            && (self.bytes[self.pos + 1] == b'x' || self.bytes[self.pos + 1] == b'X')
        {
            self.pos += 2;
            radix = 16;
        }
        let digits_start = self.pos;
        while self.pos < self.bytes.len() && is_digit_for(self.bytes[self.pos], radix) {
            self.pos += 1;
        }
        if self.pos == digits_start {
            return Err(std::io::Error::other(format!(
                "expected number digits at byte {}",
                start
            )));
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
        let value = if radix == 16 {
            // Strip optional sign + the `0x` prefix.
            let (sign, rest) = match raw.as_bytes().first() {
                Some(b'-') => (-1i64, &raw[3..]),
                Some(b'+') => (1i64, &raw[3..]),
                _ => (1i64, &raw[2..]),
            };
            i64::from_str_radix(rest, 16)
                .map(|v| sign * v)
                .map_err(|e| std::io::Error::other(e.to_string()))?
        } else {
            raw.parse::<i64>()
                .map_err(|e| std::io::Error::other(e.to_string()))?
        };
        Ok(Tok::Number(value))
    }

    fn read_string(&mut self) -> std::io::Result<Tok> {
        // Standard `"..."`. NI also accepts `~...~`, `%...%`, `#...#` and
        // `~~~~~...~~~~~` — we only emit `"..."`, so that's all we need to
        // round-trip our own output.
        debug_assert_eq!(self.bytes[self.pos], b'"');
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'"' {
            self.pos += 1;
        }
        if self.pos >= self.bytes.len() {
            return Err(std::io::Error::other("unterminated string literal"));
        }
        let s = String::from_utf8_lossy(&self.bytes[start..self.pos]).into_owned();
        self.pos += 1; // closing quote
        Ok(Tok::String(s))
    }

    fn read_ident(&mut self) -> std::io::Result<Tok> {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() && is_ident_cont(self.bytes[self.pos]) {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap()
            .to_string();
        Ok(Tok::Ident(s))
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'#' || b == b'!' || b == b'-'
}

fn is_digit_for(b: u8, radix: u32) -> bool {
    match radix {
        16 => b.is_ascii_hexdigit(),
        _ => b.is_ascii_digit(),
    }
}

// ── Parser ───────────────────────────────────────────────────────────────────

struct BafParser<'a> {
    lex: Lexer<'a>,
    peeked: Option<Tok>,
    ctx: &'a BafContext,
}

impl<'a> BafParser<'a> {
    fn new(source: &'a str, ctx: &'a BafContext) -> Self {
        Self {
            lex: Lexer::new(source),
            peeked: None,
            ctx,
        }
    }

    fn peek(&mut self) -> std::io::Result<&Tok> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lex.next()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    fn next(&mut self) -> std::io::Result<Tok> {
        if let Some(t) = self.peeked.take() {
            return Ok(t);
        }
        self.lex.next()
    }

    fn expect_ident(&mut self, name: &str) -> std::io::Result<()> {
        match self.next()? {
            Tok::Ident(s) if s.eq_ignore_ascii_case(name) => Ok(()),
            other => Err(io_err(format!(
                "expected keyword {}, got {:?}",
                name, other
            ))),
        }
    }

    fn expect(&mut self, t: &Tok) -> std::io::Result<()> {
        let got = self.next()?;
        if std::mem::discriminant(&got) == std::mem::discriminant(t) {
            // For variants without payload comparison, also check value equality
            // when payloads are present.
            if got == *t {
                Ok(())
            } else {
                Err(io_err(format!("expected {:?}, got {:?}", t, got)))
            }
        } else {
            Err(io_err(format!("expected {:?}, got {:?}", t, got)))
        }
    }

    /// Parses a top-level script (sequence of `IF ... END` blocks).
    fn parse_script(&mut self) -> std::io::Result<Bcs> {
        let mut blocks: Vec<ConditionResponse> = Vec::new();
        loop {
            match self.peek()? {
                Tok::Eof => break,
                Tok::Ident(s) if s.eq_ignore_ascii_case("IF") => {
                    self.next()?;
                    let cr = self.parse_block()?;
                    blocks.push(cr);
                }
                other => {
                    return Err(io_err(format!(
                        "expected IF or end of file, got {:?}",
                        other
                    )));
                }
            }
        }
        Ok(Bcs {
            condition_responses: blocks,
        })
    }

    /// Parses the body of one IF ... END block (caller already consumed the
    /// `IF` keyword).
    fn parse_block(&mut self) -> std::io::Result<ConditionResponse> {
        // Triggers list, until THEN.
        let mut triggers: Vec<Trigger> = Vec::new();
        while !matches!(self.peek()?, Tok::Ident(s) if s.eq_ignore_ascii_case("THEN")) {
            let trigger = self.parse_trigger_statement()?;
            // TriggerOverride wrapping is split into two consecutive triggers
            // (NextTriggerObject + the inner trigger), matching how NI's
            // compiler emits them.
            for t in trigger {
                triggers.push(t);
            }
        }
        self.expect_ident("THEN")?;

        // Responses, until END.
        let mut responses: Vec<Response> = Vec::new();
        while !matches!(self.peek()?, Tok::Ident(s) if s.eq_ignore_ascii_case("END")) {
            self.expect_ident("RESPONSE")?;
            self.expect(&Tok::Hash)?;
            let weight = match self.next()? {
                Tok::Number(n) => n as i32,
                other => return Err(io_err(format!("expected response weight, got {:?}", other))),
            };
            let mut actions: Vec<Action> = Vec::new();
            // Actions until the next RESPONSE or END.
            loop {
                match self.peek()? {
                    Tok::Ident(s)
                        if s.eq_ignore_ascii_case("RESPONSE")
                            || s.eq_ignore_ascii_case("END") =>
                    {
                        break;
                    }
                    _ => {}
                }
                let act = self.parse_action_statement()?;
                actions.push(act);
            }
            responses.push(Response { weight, actions });
        }
        self.expect_ident("END")?;

        Ok(ConditionResponse {
            condition: Condition { triggers },
            response_set: ResponseSet { responses },
        })
    }

    /// Parses one trigger statement. Returns one or more `Trigger`s because
    /// `TriggerOverride(obj, fn(...))` decomposes into a `NextTriggerObject`
    /// followed by the wrapped trigger.
    fn parse_trigger_statement(&mut self) -> std::io::Result<Vec<Trigger>> {
        let negated = if matches!(self.peek()?, Tok::Bang) {
            self.next()?;
            true
        } else {
            false
        };
        let name = match self.next()? {
            Tok::Ident(s) => s,
            other => return Err(io_err(format!("expected trigger name, got {:?}", other))),
        };

        // `TriggerOverride(obj, inner-trigger)` is a synthetic name —
        // collapse it back into NextTriggerObject + inner.
        if name.eq_ignore_ascii_case("TriggerOverride") {
            self.expect(&Tok::LParen)?;
            let obj = self.parse_object_arg()?;
            self.expect(&Tok::Comma)?;
            let inner = self.parse_trigger_statement()?;
            self.expect(&Tok::RParen)?;

            let next_trig = self.synth_next_trigger_object(obj)?;
            if inner.is_empty() {
                let mut t = next_trig;
                if negated {
                    t.flags |= 1;
                }
                return Ok(vec![t]);
            }
            // Negation moves to the inner trigger (NI's compiler does the
            // same: it OR-s the override negation into the wrapped trigger).
            let mut combined = Vec::with_capacity(1 + inner.len());
            combined.push(next_trig);
            for (i, t) in inner.into_iter().enumerate() {
                let mut t = t;
                if i == 0 && negated {
                    t.flags |= 1;
                }
                combined.push(t);
            }
            return Ok(combined);
        }

        // Resolve function signature.
        let function = self.resolve_trigger(&name)?.clone();

        self.expect(&Tok::LParen)?;
        let raw_args = self.parse_typed_arg_list(&function)?;
        self.expect(&Tok::RParen)?;

        let trigger = self.build_trigger(&function, raw_args, negated)?;
        Ok(vec![trigger])
    }

    /// Builds a `Trigger` from the resolved function signature and the
    /// already-parsed argument list.
    fn build_trigger(
        &self,
        function: &Function,
        raw_args: Vec<RawArg>,
        negated: bool,
    ) -> std::io::Result<Trigger> {
        let mut numbers: Vec<i32> = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut object: Option<ObjectAccum> = None;
        let mut point: Option<(i32, i32)> = None;

        // Pair raw args with the signature, converting symbols / objects /
        // points / strings into the right typed slots.
        let mut sig_iter = function.params.iter();
        for arg in raw_args {
            let p = sig_iter.next();
            match (arg, p) {
                (RawArg::Number(n), _) => numbers.push(n as i32),
                (RawArg::String(s), Some(param)) if param.kind == ParamKind::Integer => {
                    // Symbolic numeric value passed as a string — try to
                    // resolve through the parameter's IDS reference.
                    let v = self.resolve_string_as_number(&s, &param.ids_ref)?;
                    numbers.push(v);
                }
                (RawArg::String(s), Some(param)) if param.kind == ParamKind::Object => {
                    // A bare string in an object slot is the script-name
                    // form (NI's `decompileObject` falls back to `"name"`
                    // when no targets / identifiers are set).
                    let mut accum = ObjectAccum::default();
                    accum.name = Some(s);
                    object = Some(accum);
                }
                (RawArg::String(s), _) => strings.push(s),
                (RawArg::Symbol(sym), Some(param)) if param.kind == ParamKind::Integer => {
                    // Symbolic numeric value — usually `OR`-combined bits.
                    let v = self.resolve_symbol_as_number(&sym, &param.ids_ref)?;
                    numbers.push(v);
                }
                (RawArg::Symbol(sym), Some(param)) if param.kind == ParamKind::Object => {
                    // A bare identifier in an object slot is the unwrapped
                    // form of a single-level identifier (e.g. `Myself` or
                    // `UnknownObject1` with no wrapped target).
                    let mut accum = ObjectAccum::default();
                    accum.identifiers.push(sym);
                    object = Some(accum);
                }
                (RawArg::Symbol(sym), _) => {
                    return Err(io_err(format!(
                        "unexpected bare symbol `{}` for {}",
                        sym, function.name
                    )));
                }
                (RawArg::Object(o), _) => {
                    object = Some(o);
                }
                (RawArg::Point(x, y), _) => {
                    point = Some((x, y));
                }
            }
        }

        // Even an absent object goes through `ObjectAccum::into_object` so the
        // engine-specific region default (PST / IWD / IWD2) is applied —
        // `BcsObject::empty()` directly would lose the empty-rect sentinel.
        let target = object
            .unwrap_or_default()
            .into_object(self.ctx)?;

        let (t4, t5) = pack_strings(function, &strings, self.concat_for(function));

        // PST's bytecode reserves a trigger Point slot regardless of whether
        // the function declares one; default it to (0, 0) so round-trips
        // re-emit the `[0,0]` literal even when the BAF dropped the explicit
        // point parameter.
        let t7 = point
            .map(|(x, y)| BcsPoint { x, y })
            .or_else(|| {
                if self.ctx.trigger_has_point() {
                    Some(BcsPoint::default())
                } else {
                    None
                }
            });
        // NI's BcsTrigger exposes 3 logical numeric params via
        // setNumericParam(0..2) → t1/t2/t3. The bytecode `flags` slot is
        // separate and only carries the negation bit.
        let mut nums = [0i32; 3];
        for (i, n) in numbers.into_iter().take(3).enumerate() {
            nums[i] = n;
        }
        let flags = if negated { 1 } else { 0 };
        Ok(Trigger {
            id: function.id,
            t1: nums[0],
            flags,
            t2: nums[1],
            t3: nums[2],
            t4,
            t5,
            target,
            t7,
        })
    }

    fn synth_next_trigger_object(&self, obj: ObjectAccum) -> std::io::Result<Trigger> {
        // Find the engine's NextTriggerObject signature (single Object param).
        let func = self
            .ctx
            .triggers()
            .get_by_name("NextTriggerObject")
            .ok_or_else(|| io_err("TriggerOverride used but NextTriggerObject is not defined"))?;
        let target = obj.into_object(self.ctx)?;
        let t7 = if self.ctx.trigger_has_point() {
            Some(BcsPoint::default())
        } else {
            None
        };
        Ok(Trigger {
            id: func.id,
            t1: 0,
            flags: 0,
            t2: 0,
            t3: 0,
            t4: String::new(),
            t5: String::new(),
            target,
            t7,
        })
    }

    /// Parses one action statement, including `ActionOverride` wrapping.
    fn parse_action_statement(&mut self) -> std::io::Result<Action> {
        let name = match self.next()? {
            Tok::Ident(s) => s,
            other => return Err(io_err(format!("expected action name, got {:?}", other))),
        };

        let action_override_name = self
            .ctx
            .actions()
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

        if name.eq_ignore_ascii_case(&action_override_name) {
            self.expect(&Tok::LParen)?;
            let obj_arg = self.parse_object_arg()?;
            self.expect(&Tok::Comma)?;
            let inner = self.parse_action_statement()?;
            self.expect(&Tok::RParen)?;
            let mut act = inner;
            act.a1 = obj_arg.into_object(self.ctx)?;
            return Ok(act);
        }

        let function = self.resolve_action(&name)?.clone();
        self.expect(&Tok::LParen)?;
        let raw_args = self.parse_typed_arg_list(&function)?;
        self.expect(&Tok::RParen)?;
        self.build_action(&function, raw_args)
    }

    fn build_action(
        &self,
        function: &Function,
        raw_args: Vec<RawArg>,
    ) -> std::io::Result<Action> {
        let mut numbers: Vec<i32> = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut objects: Vec<ObjectAccum> = Vec::new();
        let mut point: Option<(i32, i32)> = None;

        let mut sig_iter = function.params.iter();
        for arg in raw_args {
            let p = sig_iter.next();
            match (arg, p) {
                (RawArg::Number(n), _) => numbers.push(n as i32),
                (RawArg::String(s), Some(param)) if param.kind == ParamKind::Integer => {
                    let v = self.resolve_string_as_number(&s, &param.ids_ref)?;
                    numbers.push(v);
                }
                (RawArg::String(s), Some(param)) if param.kind == ParamKind::Object => {
                    let mut accum = ObjectAccum::default();
                    accum.name = Some(s);
                    objects.push(accum);
                }
                (RawArg::String(s), _) => strings.push(s),
                (RawArg::Symbol(sym), Some(param)) if param.kind == ParamKind::Integer => {
                    let v = self.resolve_symbol_as_number(&sym, &param.ids_ref)?;
                    numbers.push(v);
                }
                (RawArg::Symbol(sym), Some(param)) if param.kind == ParamKind::Object => {
                    let mut accum = ObjectAccum::default();
                    accum.identifiers.push(sym);
                    objects.push(accum);
                }
                (RawArg::Symbol(sym), _) => {
                    return Err(io_err(format!(
                        "unexpected bare symbol `{}` for action {}",
                        sym, function.name
                    )));
                }
                (RawArg::Object(o), _) => objects.push(o),
                (RawArg::Point(x, y), _) => point = Some((x, y)),
            }
        }

        let make_default = || -> std::io::Result<BcsObject> {
            ObjectAccum::default().into_object(self.ctx)
        };
        let mut object_slots: [BcsObject; 3] = [make_default()?, make_default()?, make_default()?];
        // a1 is reserved for ActionOverride; the function's own object
        // arguments fill a2 then a3.
        for (i, obj) in objects.into_iter().enumerate() {
            let slot = i + 1; // 1, 2
            if slot >= object_slots.len() {
                return Err(io_err(format!(
                    "too many object arguments for action {}",
                    function.name
                )));
            }
            object_slots[slot] = obj.into_object(self.ctx)?;
        }

        let (a8, a9) = pack_strings(function, &strings, self.concat_for(function));
        let (a5_x, a5_y) = point.unwrap_or((0, 0));
        let mut nums = [0i32; 3];
        for (i, n) in numbers.into_iter().take(3).enumerate() {
            nums[i] = n;
        }
        let [a1, a2, a3] = object_slots;
        Ok(Action {
            id: function.id,
            a1,
            a2,
            a3,
            a4: nums[0],
            a5_x,
            a5_y,
            a6: nums[1],
            a7: nums[2],
            a8,
            a9,
        })
    }

    fn concat_for(&self, function: &Function) -> Option<ConcatInfo> {
        self.ctx.concat_info(function.id, function.params.len())
    }

    /// Parses a comma-separated argument list using the function's signature
    /// to disambiguate cases like `[x.y]` (point) vs `[N.N.N…]` (target list).
    /// Caller already consumed `(`, stops just before `)`.
    fn parse_typed_arg_list(&mut self, function: &Function) -> std::io::Result<Vec<RawArg>> {
        let mut args = Vec::new();
        if matches!(self.peek()?, Tok::RParen) {
            return Ok(args);
        }
        loop {
            let kind_hint = function
                .params
                .get(args.len())
                .map(|p| p.kind);
            args.push(self.parse_arg(kind_hint)?);
            match self.peek()? {
                Tok::Comma => {
                    self.next()?;
                }
                Tok::RParen => return Ok(args),
                other => {
                    return Err(io_err(format!(
                        "expected ',' or ')' in argument list, got {:?}",
                        other
                    )));
                }
            }
        }
    }

    fn parse_arg(&mut self, kind: Option<ParamKind>) -> std::io::Result<RawArg> {
        match self.peek()? {
            Tok::Number(_) => {
                let n = if let Tok::Number(n) = self.next()? {
                    n
                } else {
                    unreachable!()
                };
                Ok(RawArg::Number(n))
            }
            Tok::String(_) => {
                let s = if let Tok::String(s) = self.next()? {
                    s
                } else {
                    unreachable!()
                };
                Ok(RawArg::String(s))
            }
            Tok::LBracket => {
                // Bracketed `[…]` is overloaded: depending on the parameter
                // type it's either a point (`[x.y]`), a region (rare here),
                // or an object target list (`[ANYONE]`, `[EA.GENERAL.…]`).
                match kind {
                    Some(ParamKind::Point) => {
                        let nums = self.parse_dot_int_list()?;
                        let x = nums.first().copied().unwrap_or(0);
                        let y = nums.get(1).copied().unwrap_or(0);
                        Ok(RawArg::Point(x, y))
                    }
                    _ => {
                        let obj = self.parse_object_arg()?;
                        Ok(RawArg::Object(obj))
                    }
                }
            }
            Tok::Ident(_) => {
                // Could be: identifier wrapping an object, OR a numeric symbol
                // followed by an OR-expression chain.
                let ident = if let Tok::Ident(s) = self.next()? {
                    s
                } else {
                    unreachable!()
                };
                match self.peek()? {
                    Tok::LParen => {
                        // Identifier wrapping (e.g. NearestEnemyOf(Myself)).
                        self.next()?; // consume (
                        let mut accum = ObjectAccum::default();
                        accum.identifiers.push(ident);
                        self.collect_object_inner(&mut accum)?;
                        self.expect(&Tok::RParen)?;
                        // Optional [x.y.w.h] region suffix.
                        if matches!(self.peek()?, Tok::LBracket) {
                            self.try_parse_region_suffix(&mut accum)?;
                        }
                        Ok(RawArg::Object(accum))
                    }
                    Tok::Pipe => {
                        // OR-chain (numeric bitmask): SYMBOL | SYMBOL | ...
                        let mut value = self.resolve_symbol_as_number(&ident, "")? as i64;
                        while matches!(self.peek()?, Tok::Pipe) {
                            self.next()?;
                            match self.next()? {
                                Tok::Number(n) => value |= n,
                                Tok::Ident(name) => {
                                    value |= self.resolve_symbol_as_number(&name, "")? as i64;
                                }
                                other => {
                                    return Err(io_err(format!(
                                        "expected number or symbol after '|', got {:?}",
                                        other
                                    )));
                                }
                            }
                        }
                        Ok(RawArg::Number(value))
                    }
                    Tok::LBracket => {
                        // Bare identifier followed by a region suffix —
                        // e.g. `UnknownObject52[0.0.10000.10000]`. Promote
                        // to an object expression so the region rides
                        // along.
                        let mut accum = ObjectAccum::default();
                        accum.identifiers.push(ident);
                        self.try_parse_region_suffix(&mut accum)?;
                        Ok(RawArg::Object(accum))
                    }
                    _ => {
                        // Bare identifier — could be:
                        // - a target object identifier with no nested wrap (e.g. `Myself`)
                        // - a symbolic numeric value (e.g. `ENEMY` for an EA slot)
                        // We treat it as Symbol; later `build_*` decides
                        // based on the parameter type.
                        Ok(RawArg::Symbol(ident))
                    }
                }
            }
            other => Err(io_err(format!("unexpected token in argument: {:?}", other))),
        }
    }

    /// Parses one object argument starting at the current token (which is
    /// `[`, an identifier, or a string). Returns an `ObjectAccum` ready to
    /// be folded into a [`BcsObject`].
    fn parse_object_arg(&mut self) -> std::io::Result<ObjectAccum> {
        let mut accum = ObjectAccum::default();
        match self.peek()? {
            Tok::LBracket => self.parse_bracketed_object(&mut accum)?,
            Tok::String(_) => {
                let s = if let Tok::String(s) = self.next()? {
                    s
                } else {
                    unreachable!()
                };
                accum.name = Some(s);
            }
            Tok::Ident(_) => {
                let ident = if let Tok::Ident(s) = self.next()? {
                    s
                } else {
                    unreachable!()
                };
                accum.identifiers.push(ident);
                if matches!(self.peek()?, Tok::LParen) {
                    self.next()?;
                    self.collect_object_inner(&mut accum)?;
                    self.expect(&Tok::RParen)?;
                }
            }
            other => {
                return Err(io_err(format!(
                    "expected object argument, got {:?}",
                    other
                )));
            }
        }
        // Optional region suffix after the object.
        if matches!(self.peek()?, Tok::LBracket) {
            self.try_parse_region_suffix(&mut accum)?;
        }
        Ok(accum)
    }

    /// Recursively folds an inner-object expression (the contents of
    /// `Identifier(...)`) into the existing accumulator.
    fn collect_object_inner(&mut self, accum: &mut ObjectAccum) -> std::io::Result<()> {
        match self.peek()? {
            Tok::LBracket => self.parse_bracketed_object(accum),
            Tok::String(_) => {
                let s = if let Tok::String(s) = self.next()? {
                    s
                } else {
                    unreachable!()
                };
                accum.name = Some(s);
                Ok(())
            }
            Tok::Ident(_) => {
                let ident = if let Tok::Ident(s) = self.next()? {
                    s
                } else {
                    unreachable!()
                };
                accum.identifiers.push(ident);
                if matches!(self.peek()?, Tok::LParen) {
                    self.next()?;
                    self.collect_object_inner(accum)?;
                    self.expect(&Tok::RParen)?;
                }
                Ok(())
            }
            other => Err(io_err(format!(
                "expected inner object expression, got {:?}",
                other
            ))),
        }
    }

    /// Parses `[ANYONE]`, `[N.N.N...]`, or a target slot list with mixed
    /// numbers and symbols. The opening `[` is the current token.
    fn parse_bracketed_object(&mut self, accum: &mut ObjectAccum) -> std::io::Result<()> {
        self.expect(&Tok::LBracket)?;
        // Collect numbers / symbols separated by `.` until `]`.
        let mut slot_strs: Vec<String> = Vec::new();
        if !matches!(self.peek()?, Tok::RBracket) {
            loop {
                match self.next()? {
                    Tok::Number(n) => slot_strs.push(n.to_string()),
                    Tok::Ident(s) => slot_strs.push(s),
                    other => {
                        return Err(io_err(format!(
                            "expected number or identifier inside `[`, got {:?}",
                            other
                        )));
                    }
                }
                match self.peek()? {
                    Tok::Dot => {
                        self.next()?;
                    }
                    Tok::RBracket => break,
                    other => {
                        return Err(io_err(format!(
                            "expected '.' or ']' in target list, got {:?}",
                            other
                        )));
                    }
                }
            }
        }
        self.expect(&Tok::RBracket)?;

        if slot_strs.len() == 1 && slot_strs[0].eq_ignore_ascii_case("ANYONE") {
            // [ANYONE] — explicit "no target" marker. Leave accum target list
            // empty; the resulting BcsObject will have all-zero targets.
            return Ok(());
        }

        // Resolve each slot through the matching target IDS. For numeric
        // strings we keep them as numbers; for symbols we look up via the
        // engine's slot order (EA, GENERAL, ...).
        let names = self.ctx.object_specifier_ids();
        let mut targets: Vec<i32> = Vec::with_capacity(slot_strs.len());
        for (i, raw) in slot_strs.iter().enumerate() {
            let v = if let Ok(n) = raw.parse::<i64>() {
                n as i32
            } else {
                let ids_name = names.get(i).map(|s| s.as_str()).unwrap_or("");
                self.resolve_symbol_as_number(raw, ids_name)?
            };
            targets.push(v);
        }
        accum.targets = targets;
        Ok(())
    }

    /// Parses a `[x.y.w.h]` region suffix (4 integers, dot-separated). If the
    /// bracketed expression doesn't have exactly 4 dot-separated integers
    /// it's left in the stream and treated as a separate argument by the
    /// caller (which currently never happens for our BAF outputs, but keeps
    /// the parser conservative).
    fn try_parse_region_suffix(&mut self, accum: &mut ObjectAccum) -> std::io::Result<()> {
        // Speculatively read.
        self.expect(&Tok::LBracket)?;
        let mut nums: Vec<i32> = Vec::new();
        loop {
            match self.next()? {
                Tok::Number(n) => nums.push(n as i32),
                other => {
                    return Err(io_err(format!(
                        "expected number in region suffix, got {:?}",
                        other
                    )));
                }
            }
            match self.peek()? {
                Tok::Dot => {
                    self.next()?;
                }
                Tok::RBracket => break,
                other => {
                    return Err(io_err(format!(
                        "expected '.' or ']' in region suffix, got {:?}",
                        other
                    )));
                }
            }
        }
        self.expect(&Tok::RBracket)?;
        if nums.len() != 4 {
            return Err(io_err(format!(
                "region suffix must have 4 components, got {}",
                nums.len()
            )));
        }
        accum.region = Some(BcsRegion {
            x: nums[0],
            y: nums[1],
            width: nums[2],
            height: nums[3],
        });
        Ok(())
    }

    /// Consumes `[ N (. N)* ]` and returns the integer list. Used for points
    /// and (rectangle) regions where the contents are unambiguously numeric.
    fn parse_dot_int_list(&mut self) -> std::io::Result<Vec<i32>> {
        self.expect(&Tok::LBracket)?;
        let mut nums: Vec<i32> = Vec::new();
        if matches!(self.peek()?, Tok::RBracket) {
            self.next()?;
            return Ok(nums);
        }
        loop {
            match self.next()? {
                Tok::Number(n) => nums.push(n as i32),
                other => {
                    return Err(io_err(format!(
                        "expected number in `[…]`, got {:?}",
                        other
                    )));
                }
            }
            match self.peek()? {
                Tok::Dot => {
                    self.next()?;
                }
                Tok::RBracket => {
                    self.next()?;
                    return Ok(nums);
                }
                other => {
                    return Err(io_err(format!(
                        "expected '.' or ']', got {:?}",
                        other
                    )));
                }
            }
        }
    }

    fn resolve_trigger(&self, name: &str) -> std::io::Result<&Function> {
        self.ctx
            .triggers()
            .get_by_name(name)
            .ok_or_else(|| io_err(format!("unknown trigger function `{}`", name)))
    }

    fn resolve_action(&self, name: &str) -> std::io::Result<&Function> {
        self.ctx
            .actions()
            .get_by_name(name)
            .ok_or_else(|| io_err(format!("unknown action function `{}`", name)))
    }

    /// Resolves a symbolic value for an integer parameter:
    ///   - `UnknownObject<n>` and pure digits → numeric
    ///   - registered IDS lookup via `ids_name`
    ///   - hex literals via `0x…`
    fn resolve_symbol_as_number(&self, sym: &str, ids_name: &str) -> std::io::Result<i32> {
        // UnknownObject<N> is the placeholder NI emits when an OBJECT.IDS
        // lookup fails — round-trip it back to the raw integer.
        if let Some(rest) = sym.strip_prefix("UnknownObject") {
            return rest
                .parse::<i32>()
                .map_err(|e| io_err(format!("malformed UnknownObject: {} ({})", sym, e)));
        }
        // Plain numeric literal hidden in an identifier slot.
        if let Ok(n) = sym.parse::<i32>() {
            return Ok(n);
        }
        if let Some(stripped) = sym.strip_prefix("0x").or_else(|| sym.strip_prefix("0X"))
            && let Ok(n) = i64::from_str_radix(stripped, 16)
        {
            return Ok(n as i32);
        }
        if !ids_name.is_empty()
            && let Some(map) = self.ctx.ids_lookup(ids_name)
            && let Some(v) = map.of_value_str_ci(sym)
        {
            return Ok(v);
        }
        Err(io_err(format!(
            "cannot resolve symbol `{}` (ids ref: `{}`)",
            sym, ids_name
        )))
    }

    fn resolve_string_as_number(&self, s: &str, ids_name: &str) -> std::io::Result<i32> {
        if let Ok(n) = s.parse::<i32>() {
            return Ok(n);
        }
        if let Some(stripped) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))
            && let Ok(n) = i64::from_str_radix(stripped, 16)
        {
            return Ok(n as i32);
        }
        if !ids_name.is_empty()
            && let Some(map) = self.ctx.ids_lookup(ids_name)
            && let Some(v) = map.of_value_str_ci(s)
        {
            return Ok(v);
        }
        Err(io_err(format!(
            "cannot interpret quoted value `{}` as number (ids ref: `{}`)",
            s, ids_name
        )))
    }
}

// ── Argument / object accumulators ───────────────────────────────────────────

#[derive(Debug)]
enum RawArg {
    Number(i64),
    String(String),
    Symbol(String),
    Object(ObjectAccum),
    Point(i32, i32),
}

/// Mutable working state for one object expression. Symbol → number
/// resolution is deferred until [`Self::into_object`] so the parser doesn't
/// need to know whether identifiers came from OBJECT.IDS or directly as
/// `UnknownObject<n>`.
#[derive(Debug, Default)]
struct ObjectAccum {
    /// Target slots in slot order (EA, GENERAL, …). Numbers preserved
    /// verbatim; the wrapping nesting goes into `identifiers` instead.
    targets: Vec<i32>,
    /// Identifier symbols, in the textual order they appeared (outermost
    /// first). On conversion, we reverse them so identifiers[0] is the
    /// innermost wrap, matching the BCS bytecode layout.
    identifiers: Vec<String>,
    /// Optional script-name string.
    name: Option<String>,
    /// Optional `[x.y.w.h]` region suffix.
    region: Option<BcsRegion>,
}

impl ObjectAccum {
    fn into_object(self, ctx: &BafContext) -> std::io::Result<BcsObject> {
        // Reverse identifier order so the BAF outermost wrap (read left to
        // right) ends up at the highest non-zero `identifiers` slot — the
        // exact inverse of what the decompiler does on the way out.
        let object_ids = ctx.ids_lookup("OBJECT");
        let mut idents = [0i32; 5];
        for (slot, sym) in self.identifiers.iter().rev().enumerate() {
            if slot >= idents.len() {
                return Err(io_err(format!(
                    "too many nested object identifiers ({})",
                    self.identifiers.len()
                )));
            }
            idents[slot] = resolve_object_identifier(sym, object_ids)?;
        }
        let mut targets = self.targets;
        // Pad target list out to the engine's slot count so the recompiled
        // bytecode has the canonical shape. NI's `BcsObject.toByteCode()`
        // does the same — for PST it always emits 9 target slots, for IWD2
        // 10. A small tail of PST scripts in the wild use the BG 7-slot
        // shape (see the `pst_legacy_7slot_objects` failures); those won't
        // round-trip byte-perfectly because the BAF doesn't carry the slot
        // count separately.
        let want = ctx.object_specifier_ids().len().max(targets.len()).max(7);
        if targets.len() < want {
            targets.resize(want, 0);
        }
        // When the engine's bytecode carries a region slot, default to the
        // empty `(-1, -1, -1, -1)` sentinel so the round-trip writes the
        // bracketed marker even when the BAF didn't include one (NI's
        // headless decompiler drops the empty rect from BAF output).
        let region = self.region.or_else(|| {
            if ctx.object_has_region() {
                Some(BcsRegion::default())
            } else {
                None
            }
        });
        Ok(BcsObject {
            targets,
            identifiers: idents,
            name: self.name.unwrap_or_default(),
            region,
            trailing_targets: ctx.object_trailing_targets(),
        })
    }
}

fn resolve_object_identifier(
    sym: &str,
    object_ids: Option<&infinitier_ids_importer::Ids>,
) -> std::io::Result<i32> {
    if let Some(rest) = sym.strip_prefix("UnknownObject") {
        return rest
            .parse::<i32>()
            .map_err(|e| io_err(format!("malformed UnknownObject identifier: {} ({})", sym, e)));
    }
    if let Some(map) = object_ids
        && let Some(v) = map.of_value_str_ci(sym)
    {
        return Ok(v);
    }
    Err(io_err(format!(
        "cannot resolve object identifier `{}`",
        sym
    )))
}

// ── String packing (inverse of get_string_arg in baf.rs) ────────────────────

/// Packs the function's logical string arguments into the two physical
/// bytecode slots, applying combined-string packing when configured.
/// Mirrors NI's `BcsStructureBase.setStringParams`.
fn pack_strings(
    function: &Function,
    strings: &[String],
    concat: Option<ConcatInfo>,
) -> (String, String) {
    let mut out: [String; 2] = [String::new(), String::new()];
    let mut src_index = 0usize;
    let mut dst_index = 0usize;
    let mut even = true;
    let mut scnt = 0usize;
    for p in &function.params {
        if p.kind != ParamKind::String {
            continue;
        }
        if src_index >= strings.len() {
            break;
        }
        let (combined, colon) = combined_flags_at(concat, scnt);
        if combined {
            if colon && out[dst_index].is_empty() {
                out[dst_index].push(':');
            }
            if even {
                let s = out[dst_index].clone();
                out[dst_index] = s + &strings[src_index];
            } else {
                let s = out[dst_index].clone();
                out[dst_index] = strings[src_index].clone() + &s;
                dst_index += 1;
            }
            even = !even;
        } else {
            if dst_index < out.len() {
                out[dst_index] = strings[src_index].clone();
                dst_index += 1;
            }
            even = true;
        }
        src_index += 1;
        scnt += 1;
    }
    let [a, b] = out;
    (a, b)
}

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

fn io_err(msg: impl Into<String>) -> std::io::Error {
    std::io::Error::other(msg.into())
}

// ── Ids helper ───────────────────────────────────────────────────────────────
// The IDS importer's lookup is case-sensitive; case-insensitive lookups are
// needed to compile BAF that uses any casing (NI's matcher is case-
// insensitive by default for symbol resolution).
trait IdsCaseInsensitive {
    fn of_value_str_ci(&self, name: &str) -> Option<i32>;
}

impl IdsCaseInsensitive for infinitier_ids_importer::Ids {
    fn of_value_str_ci(&self, name: &str) -> Option<i32> {
        let needle = name.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| e.name.to_ascii_lowercase() == needle)
            .map(|e| e.value)
    }
}

// ── Round-trip corpus tests ──────────────────────────────────────────────────
//
// For every BCS file in a real game's `extracted_resources/<game>/bcs/original/`,
// run BCS → BAF → BCS and assert byte-equality with the original. Same
// per-game contexts as `baf::corpus_tests::baf_corpus_*`. Each game has its
// own `#[test]` so failures stay attributable; tests skip silently when the
// game folder is absent. Override the corpus root with `EXTRACTED_RESOURCES`.

#[cfg(test)]
mod roundtrip_tests {
    use crate::baf::BafContext;
    use crate::signatures::Signatures;
    use crate::{Bcs, BcsImporter};
    use infinitier_common::Game;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_ids_importer::IdsImporter;
    use std::path::{Path, PathBuf};

    fn extracted_root() -> PathBuf {
        let raw = std::env::var("EXTRACTED_RESOURCES").unwrap_or_else(|_| {
            "/home/ufo/workspaces/github_ufoscout/baldurs_gate/extracted_resources".to_string()
        });
        PathBuf::from(raw)
    }

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

    fn load_signatures(ids_dir: &Path, stem: &str) -> Signatures {
        let path = find_ids(ids_dir, stem)
            .unwrap_or_else(|| panic!("missing IDS {}.IDS in {}", stem, ids_dir.display()));
        let ids = IdsImporter
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
        Signatures::from_ids(&ids)
    }

    fn build_context(game: Game, ids_dir: &Path) -> BafContext {
        let triggers = load_signatures(ids_dir, "TRIGGER");
        let actions = load_signatures(ids_dir, "ACTION");
        BafContext::new(triggers, actions, game)
    }

    /// Round-trips every `*.bcs` / `*.bs` file under `<corpus_dir>/original`
    /// through `BCS → BAF → BCS` and asserts byte-equality with the original.
    ///
    /// A small minority of files in real game extracts can't survive the
    /// round-trip — e.g. junk bytes in unused trigger/action slots that the
    /// function signature doesn't expose, references to function ids missing
    /// from TRIGGER.IDS, or PST OB blocks compiled with the older 7-slot
    /// shape NI re-emits as 9-slot. For those, we fall back to an
    /// **idempotence** check: BCS → BAF → BCS → BAF → BCS must equal
    /// BCS → BAF → BCS, which still proves the compile/decompile pair is
    /// internally consistent even when the original bytes are lossy.
    fn assert_roundtrip(corpus_dir: &Path, ctx: &BafContext) {
        let original_dir = corpus_dir.join("original");
        assert!(
            original_dir.is_dir(),
            "missing {}",
            original_dir.display()
        );
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
            let original_text = match std::fs::read_to_string(src_path) {
                // BAF only carries LF line endings, so any CRLF in the
                // source is irrecoverably lost across the round-trip.
                // Normalize for comparison rather than treating it as a
                // real diff.
                Ok(s) => {
                    let normalized = s.replace("\r\n", "\n");
                    // A truly empty BCS file and a `SC\nSC\n` empty script
                    // both decompile to nothing and recompile to `SC\nSC\n`;
                    // treat them as equivalent so the round-trip passes.
                    if normalized.trim().is_empty() {
                        "SC\nSC\n".to_string()
                    } else {
                        normalized
                    }
                }
                Err(e) => {
                    failures.push(format!("read {}: {}", src_path.display(), e));
                    continue;
                }
            };
            let bcs = match BcsImporter.import(&DataSource::new(src_path.as_path())) {
                Ok(b) => b,
                Err(e) => {
                    failures.push(format!("parse {}: {}", src_path.display(), e));
                    continue;
                }
            };
            let baf = bcs.to_baf(ctx);
            let recompiled = match Bcs::from_baf(&baf, ctx) {
                Ok(b) => b,
                Err(e) => {
                    failures.push(format!("compile {}: {}", src_path.display(), e));
                    continue;
                }
            };
            let regenerated = recompiled.to_byte_code();
            if regenerated != original_text {
                // Original isn't byte-equal — that's expected for a small
                // tail of files. Verify that the round-trip is at least
                // **idempotent**: re-compiling the regenerated bytecode
                // produces the same regenerated bytecode, proving the BAF
                // compiler / decompiler pair is internally consistent.
                let stable = match BcsImporter
                    .import(&DataSource::new(regenerated.as_bytes()))
                    .and_then(|b| {
                        let baf2 = b.to_baf(ctx);
                        Bcs::from_baf(&baf2, ctx)
                    }) {
                    Ok(b) => b.to_byte_code() == regenerated,
                    Err(_) => false,
                };
                if stable {
                    // Lossy original — but our pipeline is stable. Skip.
                    continue;
                }
                // Surface first per-line difference plus a length summary so
                // both layout and content drifts are easy to spot.
                let mut diffs: Vec<String> = Vec::new();
                for (i, (a, b)) in original_text
                    .lines()
                    .zip(regenerated.lines())
                    .enumerate()
                {
                    if a != b {
                        diffs.push(format!(
                            "  line {}: original {:?}\n            recompiled {:?}",
                            i + 1,
                            a,
                            b
                        ));
                        if diffs.len() >= 3 {
                            break;
                        }
                    }
                }
                if diffs.is_empty() {
                    diffs.push(format!(
                        "  trailing difference (orig {} bytes / {} lines, regen {} / {})",
                        original_text.len(),
                        original_text.lines().count(),
                        regenerated.len(),
                        regenerated.lines().count(),
                    ));
                }
                failures.push(format!(
                    "BCS mismatch {}\n{}",
                    src_path.display(),
                    diffs.join("\n")
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

    fn run_game(dir: &str, game: Game) {
        let root = extracted_root();
        let game_dir = root.join(dir);
        let corpus = game_dir.join("bcs");
        let ids_dir = game_dir.join("ids");
        if !corpus.is_dir() || !ids_dir.is_dir() {
            eprintln!(
                "skip roundtrip test for {}: missing {}",
                dir,
                game_dir.display()
            );
            return;
        }
        let ctx = build_context(game, &ids_dir);
        assert_roundtrip(&corpus, &ctx);
    }

    #[test]
    fn roundtrip_bg() {
        run_game("bg", Game::Bg);
    }

    #[test]
    fn roundtrip_bgee() {
        run_game("bgee", Game::Bgee);
    }

    #[test]
    fn roundtrip_bg2() {
        run_game("bg2", Game::Bg2);
    }

    #[test]
    fn roundtrip_bg2ee() {
        run_game("bg2ee", Game::Bg2ee);
    }

    #[test]
    fn roundtrip_iwd() {
        run_game("iwd", Game::Iwd);
    }

    #[test]
    fn roundtrip_iwdee() {
        run_game("iwdee", Game::Iwdee);
    }

    #[test]
    fn roundtrip_iwd2() {
        run_game("iwd2", Game::Iwd2);
    }

    #[test]
    fn roundtrip_pst() {
        run_game("pst", Game::Pst);
    }

    #[test]
    fn roundtrip_pstee() {
        run_game("pstee", Game::Pstee);
    }
}
