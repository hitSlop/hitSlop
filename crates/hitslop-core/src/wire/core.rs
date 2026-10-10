//! Document intents and publications shared by native and WASM owners.
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Segment {
    Key(String),
    Id { id: String },
    Index { index: usize },
}
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Anchor {
    Before { before: String },
    After { after: String },
}
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Hunk {
    Retain { retain: usize },
    Insert { insert: String },
    Delete { delete: usize },
}
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PatchOp {
    Set {
        path: Vec<Segment>,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: Value,
    },
    Text {
        path: Vec<Segment>,
        delta: Vec<Hunk>,
    },
    Remove {
        path: Vec<Segment>,
    },
    InsertRow {
        path: Vec<Segment>,
        index: usize,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: Value,
    },
    DeleteRow {
        path: Vec<Segment>,
        id: String,
    },
    MoveRow {
        path: Vec<Segment>,
        id: String,
        index: usize,
    },
}
impl PatchOp {
    pub fn path(&self) -> &[Segment] {
        match self {
            Self::Set { path, .. } => path,
            Self::Text { path, .. } => path,
            Self::Remove { path, .. } => path,
            Self::InsertRow { path, .. } => path,
            Self::DeleteRow { path, .. } => path,
            Self::MoveRow { path, .. } => path,
        }
    }
}
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Debug, Serialize)]
pub struct Publication {
    pub previous: u64,
    pub sequence: u64,
    pub version: String,
    pub ops: Vec<PatchOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<std::collections::BTreeMap<String, String>>,
}
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Intent {
    Set {
        path: Vec<Segment>,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: Value,
        from: Option<String>,
        selection: Option<Selection>,
    },
    Insert {
        path: Vec<Segment>,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: Value,
        id: Option<String>,
        at: Option<Anchor>,
        index: Option<usize>,
    },
    Remove {
        path: Vec<Segment>,
        id: Option<String>,
        index: Option<usize>,
        count: Option<usize>,
    },
    Move {
        path: Vec<Segment>,
        id: String,
        at: Option<Anchor>,
    },
    Clear {
        path: Vec<Segment>,
    },
    Increment {
        path: Vec<Segment>,
        by: i64,
    },
    Replace {
        path: Vec<Segment>,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        value: Value,
    },
    SetTheme {
        values: std::collections::BTreeMap<String, Option<String>>,
        replace: Option<bool>,
    },
    ImportTheme {
        file: String,
    },
}
impl Intent {
    /// The data path; palette intents have none.
    pub fn path(&self) -> &[Segment] {
        match self {
            Self::Set { path, .. } => path,
            Self::Insert { path, .. } => path,
            Self::Remove { path, .. } => path,
            Self::Move { path, .. } => path,
            Self::Clear { path, .. } => path,
            Self::Increment { path, .. } => path,
            Self::Replace { path, .. } => path,
            Self::SetTheme { .. } => &[],
            Self::ImportTheme { .. } => &[],
        }
    }
}
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    #[serde(default, deserialize_with = "super::present_option", skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ifVersion")]
    pub if_version: Option<String>,
    pub intents: Vec<Intent>,
}
impl Batch {
    /// JSON adapters decode at their boundary; the owner carries this typed value.
    pub fn decode(input: &str) -> crate::Result<Self> {
        crate::parse(input)
    }
    pub(crate) fn check_size(&self, limit: usize) -> crate::Result<()> {
        check_json_size(self, limit)
    }
}

/// Counts encoded bytes without allocating a second payload or decoding it again.
pub(crate) fn check_json_size(value: &impl Serialize, limit: usize) -> crate::Result<()> {
    struct Budget(usize);
    impl std::io::Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.checked_sub(bytes.len()).ok_or_else(|| std::io::Error::other("JSON exceeds size limit"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget(limit), value)
        .map_err(|_| crate::err(crate::Code::TooLarge, "JSON exceeds size limit"))
}

/// The immutable reading shared by page, native, and command responses.
#[derive(Debug, Serialize)]
pub struct Reading {
    pub version: String,
    pub value: Value,
    pub theme: std::collections::BTreeMap<String, String>,
}
impl Reading {
    pub fn to_json(&self) -> String {
        crate::encode(self)
    }
    pub fn sequenced(self, sequence: u64) -> OwnerState {
        OwnerState { sequence, version: self.version, value: self.value, theme: self.theme }
    }
}

/// A text selection in UTF-16 offsets.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}
/// A shared theme file: the template it was made for and its palette.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields, export_to = "core.generated.ts"))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFile {
    pub template: String,
    pub values: std::collections::BTreeMap<String, String>,
}
/// A snapshot delivered to the page after its publication sequence.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "core.generated.ts"))]
pub struct OwnerState {
    pub sequence: u64,
    pub version: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub value: Value,
    pub theme: std::collections::BTreeMap<String, String>,
}
