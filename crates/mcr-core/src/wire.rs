use crate::hunk::ChangeRegion;
use serde::{Deserialize, Serialize};

/// One display row across the three panes. A `None` field renders as alignment
/// filler in that pane so corresponding lines stay horizontally aligned (FR-005).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignRow {
    pub local: Option<usize>,
    pub result: Option<usize>,
    pub incoming: Option<usize>,
    /// Owning change region id, if this row belongs to one.
    pub hunk: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panes {
    pub local: Vec<String>,
    pub result: Vec<String>,
    pub incoming: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionStatus {
    pub total_hunks: usize,
    pub remaining_conflicts: usize,
    pub fully_resolved: bool,
}

/// Full session view state handed to the UI. The UI renders this read-only and
/// dispatches intents back; it derives no merge state itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionModel {
    pub session_id: String,
    pub panes: Panes,
    pub alignment: Vec<AlignRow>,
    pub hunks: Vec<ChangeRegion>,
    pub status: ResolutionStatus,
}
