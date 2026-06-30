use serde::{Deserialize, Serialize};

/// Half-open line range `[start, end)` in some pane's own line coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

impl LineRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && line < self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Local,
    Incoming,
}

/// Which version(s) introduced the change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Local,
    Incoming,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Added,
    Removed,
    Modified,
    Conflicting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pane {
    Local,
    Result,
    Incoming,
}

/// Lifecycle state of a change region inside the merged result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HunkState {
    Unresolved,
    Applied { from: Side },
    Rejected,
    ManuallyEdited { lines: Vec<String> },
}

/// Exact word/character span within a changed line (intra-line highlight).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntraLineSpan {
    pub pane: Pane,
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// A contiguous differing block, anchored to each pane's line coordinates and
/// to the live result document via `result_range`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeRegion {
    pub id: usize,
    pub origin: Origin,
    pub category: Category,
    pub local_range: LineRange,
    pub incoming_range: LineRange,
    /// Span occupied in the materialized result document.
    pub result_range: LineRange,
    pub word_spans: Vec<IntraLineSpan>,
    pub state: HunkState,
}
