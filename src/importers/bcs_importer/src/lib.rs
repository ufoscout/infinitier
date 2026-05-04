use infinitier_datasource::{DataSource, Importer};
use log::debug;
use serde::{Deserialize, Serialize};

/// A BCS script file importer.
pub struct BcsImporter;

impl Importer for BcsImporter {
    type T = Bcs;

    fn import(&self, source: &DataSource) -> std::io::Result<Bcs> {
        let mut reader = source.reader()?;
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let text = String::from_utf8_lossy(&buf);
        let mut stream = BcsStream::new(&text);
        let bcs = parse_bcs(&mut stream)?;
        debug!(
            "Loaded BCS: {} condition-response blocks",
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
/// `flags & 1` means the trigger result is negated.
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
/// For BG/BG2 the 12 numeric values split as:
/// * `targets[0..7]`     — EA, General, Race, Class, Specific, Gender, Alignment
/// * `identifiers[0..5]` — OBJECT.IDS nesting levels
///
/// `name` is the script name string.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BcsObject {
    pub targets: [i32; 7],
    pub identifiers: [i32; 5],
    pub name: String,
}

// ── Token stream ─────────────────────────────────────────────────────────────

struct BcsStream<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BcsStream<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            data: text.as_bytes(),
            pos: 0,
        }
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
        let s = String::from_utf8_lossy(&self.data[start..self.pos]).into_owned();
        self.pos += 1; // consume closing "
        Ok(s)
    }

    fn is_eos(&self) -> bool {
        self.pos >= self.data.len()
    }
}

// ── Parser ────────────────────────────────────────────────────────────────────

fn parse_bcs(s: &mut BcsStream<'_>) -> std::io::Result<Bcs> {
    s.expect("SC")?;
    let mut condition_responses = Vec::new();
    while !s.is_eos() && !s.try_skip("SC") {
        condition_responses.push(parse_condition_response(s)?);
    }
    Ok(Bcs { condition_responses })
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
    s.expect("TR")?;
    let id = s.read_i32()?;
    let t1 = s.read_i32()?;
    let flags = s.read_i32()?;
    let t2 = s.read_i32()?;
    let t3 = s.read_i32()?;
    let t4 = s.read_string()?;
    let t5 = s.read_string()?;
    let target = parse_object(s)?;
    s.expect("TR")?;
    Ok(Trigger {
        id,
        flags,
        t1,
        t2,
        t3,
        t4,
        t5,
        target,
    })
}

fn parse_action(s: &mut BcsStream<'_>) -> std::io::Result<Action> {
    s.expect("AC")?;
    let id = s.read_i32()?;
    let a1 = parse_object(s)?;
    let a2 = parse_object(s)?;
    let a3 = parse_object(s)?;
    let a4 = s.read_i32()?;
    let a5_x = s.read_i32()?;
    let a5_y = s.read_i32()?;
    let a6 = s.read_i32()?;
    let a7 = s.read_i32()?;
    let a8 = s.read_string()?;
    let a9 = s.read_string()?;
    s.expect("AC")?;
    Ok(Action {
        id,
        a1,
        a2,
        a3,
        a4,
        a5_x,
        a5_y,
        a6,
        a7,
        a8,
        a9,
    })
}

fn parse_object(s: &mut BcsStream<'_>) -> std::io::Result<BcsObject> {
    s.expect("OB")?;
    let mut nums: Vec<i32> = Vec::new();
    loop {
        match s.peek() {
            Some(b'-') | Some(b'0'..=b'9') => nums.push(s.read_i32()?),
            _ => break,
        }
    }
    let name = s.read_string()?;
    s.expect("OB")?;

    // Last 5 numbers are OBJECT.IDS identifiers; the rest are target specifiers.
    let n = nums.len();
    let split = n.saturating_sub(5);

    let mut targets = [0i32; 7];
    let mut identifiers = [0i32; 5];
    for (i, &v) in nums[..split].iter().enumerate().take(7) {
        targets[i] = v;
    }
    for (i, &v) in nums[split..].iter().enumerate().take(5) {
        identifiers[i] = v;
    }

    Ok(BcsObject {
        targets,
        identifiers,
        name,
    })
}


impl Bcs {
    /// Serializes this script to the BCS byte-code text format (the SC/CR/CO/TR/… encoding
    /// stored in game files). Parsing the returned string produces an equal `Bcs`.
    pub fn to_byte_code(&self) -> String {
        let mut out = String::new();
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
    out.push_str(&format!(
        "{} {} {} {} {} \"{}\" \"{}\" OB\n",
        t.id, t.t1, t.flags, t.t2, t.t3, t.t4, t.t5
    ));
    push_object_content(out, &t.target);
    out.push_str("TR\n");
}

fn push_response(out: &mut String, r: &Response) {
    out.push_str("RE\n");
    out.push_str(&r.weight.to_string());
    for action in &r.actions {
        out.push_str("AC\n");
        push_action(out, action);
    }
    out.push_str("RE\n");
}

fn push_action(out: &mut String, a: &Action) {
    out.push_str(&format!("{}OB\n", a.id));
    push_object_content(out, &a.a1);
    out.push_str("OB\n");
    push_object_content(out, &a.a2);
    out.push_str("OB\n");
    push_object_content(out, &a.a3);
    // no space between a7 and the opening quote — matches the game format
    out.push_str(&format!(
        "{} {} {} {} {}\"{}\" \"{}\" AC\n",
        a.a4, a.a5_x, a.a5_y, a.a6, a.a7, a.a8, a.a9
    ));
}

fn push_object_content(out: &mut String, obj: &BcsObject) {
    // 12 integers + closing "name"OB on one line; OB is adjacent to the closing quote
    out.push_str(&format!(
        "{} {} {} {} {} {} {} {} {} {} {} {} \"{}\"OB\n",
        obj.targets[0],
        obj.targets[1],
        obj.targets[2],
        obj.targets[3],
        obj.targets[4],
        obj.targets[5],
        obj.targets[6],
        obj.identifiers[0],
        obj.identifiers[1],
        obj.identifiers[2],
        obj.identifiers[3],
        obj.identifiers[4],
        obj.name
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path, parse_json_file};

    #[test]
    fn test_parse_empty_script() {
        let mut s = BcsStream::new("SC\nSC\n");
        let bcs = parse_bcs(&mut s).unwrap();
        assert_eq!(bcs.condition_responses.len(), 0);
    }

    #[test]
    fn test_parse_simple_object_all_zeros() {
        let src = "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\n";
        let mut s = BcsStream::new(src);
        let obj = parse_object(&mut s).unwrap();
        assert_eq!(obj.targets, [0; 7]);
        assert_eq!(obj.identifiers, [0; 5]);
        assert_eq!(obj.name, "");
    }

    #[test]
    fn test_parse_object_with_ea_and_identifiers() {
        // ea=30 in targets, identifiers=[1,12,0,0,0]
        let src = "OB\n30 0 0 0 0 0 0 1 12 0 0 0 \"\"OB\n";
        let mut s = BcsStream::new(src);
        let obj = parse_object(&mut s).unwrap();
        assert_eq!(obj.targets, [30, 0, 0, 0, 0, 0, 0]);
        assert_eq!(obj.identifiers, [1, 12, 0, 0, 0]);
        assert_eq!(obj.name, "");
    }

    #[test]
    fn test_parse_object_with_name() {
        let src = "OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"Caveentrance\"OB\n";
        let mut s = BcsStream::new(src);
        let obj = parse_object(&mut s).unwrap();
        assert_eq!(obj.name, "Caveentrance");
    }

    #[test]
    fn test_parse_trigger() {
        let src = "TR\n47 111 0 0 0 \"\" \"\" OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\nTR\n";
        let mut s = BcsStream::new(src);
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
        let mut s = BcsStream::new(src);
        let t = parse_trigger(&mut s).unwrap();
        assert_eq!(t.id, 16395);
        assert_eq!(t.t1, 255);
        assert_eq!(t.flags, 1);
        assert_eq!(t.target.identifiers, [1, 0, 0, 0, 0]);
    }

    #[test]
    fn test_parse_trigger_with_string_param() {
        let src =
            "TR\n16399 1 0 0 0 \"GLOBALReturnedOutside\" \"\" OB\n0 0 0 0 0 0 0 0 0 0 0 0 \"\"OB\nTR\n";
        let mut s = BcsStream::new(src);
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
        let mut s = BcsStream::new(src);
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
        let mut s = BcsStream::new(src);
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

            let actual = BcsImporter
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
                let bcs_from_bytes = BcsImporter
                    .import(&DataSource::new(bcs_bytes_generated.as_bytes()))
                    .unwrap_or_else(|e| panic!("cannot import {}: {e}", bcs_path.display()));
                assert_eq!(bcs_from_bytes, actual, "BCS mismatch for {}", bcs_path.display());

                let bcs_from_file: String = std::fs::read_to_string(bcs_path).unwrap();
                assert_eq!(bcs_from_file, bcs_bytes_generated);
            }
        }
    }

}
