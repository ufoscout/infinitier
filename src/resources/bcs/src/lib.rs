#![doc = include_str!("../readme.md")]

use serde::{Deserialize, Serialize};

pub mod baf;
pub mod baf_compile;
mod exporter;
mod importer;
pub mod signatures;

pub use exporter::BcsExporter;
pub use importer::BcsImporter;

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
