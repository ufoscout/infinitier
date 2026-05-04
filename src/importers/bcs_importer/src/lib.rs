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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::{get_all_in_folder_by_extension, get_assets_path, parse_json_file};

    // ── unit tests ────────────────────────────────────────────────────────────

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

    // ── integration tests ─────────────────────────────────────────────────────

    #[test]
    fn test_all_bcs_files() {
        let bcs_folder = get_assets_path().join("resources/BCS");
        let paths = get_all_in_folder_by_extension(&bcs_folder, "bcs");
        assert!(!paths.is_empty(), "no BCS files found");

        for bcs_path in paths {
            let json_path = bcs_path.with_extension("json");
            let expected: Bcs = parse_json_file(&json_path);
            let actual = BcsImporter
                .import(&DataSource::new(bcs_path.as_path()))
                .unwrap_or_else(|e| panic!("cannot import {}: {e}", bcs_path.display()));
            assert_eq!(actual, expected, "BCS mismatch for {}", bcs_path.display());
        }
    }

    #[test]
    fn test_parse_randwlkc() {
        let path = get_assets_path().join("resources/BCS/randwlkc.bcs");
        let bcs = BcsImporter
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(bcs.condition_responses.len(), 1);
        let cr = &bcs.condition_responses[0];
        assert_eq!(cr.condition.triggers.len(), 1);
        assert_eq!(cr.condition.triggers[0].id, 16419);
        assert_eq!(cr.response_set.responses.len(), 1);
        assert_eq!(cr.response_set.responses[0].weight, 100);
        assert_eq!(cr.response_set.responses[0].actions.len(), 1);
        assert_eq!(cr.response_set.responses[0].actions[0].id, 200);
    }

    #[test]
    fn test_parse_dplayer3() {
        let path = get_assets_path().join("resources/BCS/dplayer3.bcs");
        let bcs = BcsImporter
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(bcs.condition_responses.len(), 1);
        let cr = &bcs.condition_responses[0];
        assert_eq!(cr.condition.triggers.len(), 2);
        let t1 = &cr.condition.triggers[0];
        assert_eq!(t1.id, 16399);
        assert_eq!(t1.flags, 1); // negated
        assert_eq!(t1.t4, "GLOBALSeenBlockade");
        let t2 = &cr.condition.triggers[1];
        assert_eq!(t2.id, 16412);
        assert_eq!(t2.target.name, "Caveentrance");
        assert_eq!(cr.response_set.responses[0].actions[0].id, 30);
        assert_eq!(cr.response_set.responses[0].actions[0].a4, 1);
        assert_eq!(
            cr.response_set.responses[0].actions[0].a8,
            "GLOBALSeenBlockade"
        );
    }

    #[test]
    fn test_parse_ar0100() {
        let path = get_assets_path().join("resources/BCS/ar0100.bcs");
        let bcs = BcsImporter
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(bcs.condition_responses.len(), 2);
        let cr0 = &bcs.condition_responses[0];
        assert_eq!(cr0.condition.triggers[0].id, 16399);
        assert_eq!(cr0.condition.triggers[0].t4, "GLOBALReturnedOutside");
        assert_eq!(cr0.response_set.responses[0].actions.len(), 3);
        let a2 = &cr0.response_set.responses[0].actions[2];
        assert_eq!(a2.id, 30);
        assert_eq!(a2.a4, 2);
        assert_eq!(a2.a8, "GLOBALReturnedOutside");
    }

    #[test]
    fn test_parse_foozle() {
        let path = get_assets_path().join("resources/BCS/foozle.bcs");
        let bcs = BcsImporter
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        assert_eq!(bcs.condition_responses.len(), 4);

        let cr1 = &bcs.condition_responses[0];
        assert_eq!(cr1.condition.triggers[0].id, 47);
        assert_eq!(cr1.condition.triggers[0].t1, 111);
        assert_eq!(cr1.response_set.responses[0].actions[0].id, 22);

        let cr2 = &bcs.condition_responses[1];
        assert_eq!(cr2.condition.triggers.len(), 2);
        assert_eq!(cr2.condition.triggers[0].target.targets[0], 30);
        assert_eq!(cr2.condition.triggers[1].flags, 1);

        let cr3 = &bcs.condition_responses[2];
        assert_eq!(cr3.response_set.responses[0].actions[0].a4, 30);

        let cr4 = &bcs.condition_responses[3];
        assert_eq!(cr4.response_set.responses[0].actions[0].id, 3);
    }
}
