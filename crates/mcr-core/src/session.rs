use crate::diff::{diff3, word_spans, Region, WhitespaceMode};
use crate::hunk::{
    Category, ChangeRegion, HunkState, IntraLineSpan, LineRange, Origin, Pane, Side,
};
use crate::ops::{HunkChange, ManualChange, Operation, OperationLog};
use crate::wire::{AlignRow, Panes, ResolutionStatus, SessionModel};

fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n').map(|s| s.to_string()).collect()
}

#[derive(Clone)]
struct ChangeMeta {
    region_idx: usize,
    origin: Origin,
    category: Category,
}

pub struct MergeSession {
    pub id: String,
    base: Vec<String>,
    local: Vec<String>,
    incoming: Vec<String>,
    mode: WhitespaceMode,
    regions: Vec<Region>,
    changes: Vec<ChangeMeta>,
    states: Vec<HunkState>,
    /// Free-form manual edit of the whole result. When set it is the authoritative
    /// result text (what gets written out); any hunk gizmo operation clears it so
    /// the projection becomes authoritative again.
    manual: Option<Vec<String>>,
    log: OperationLog,
}

impl MergeSession {
    pub fn open(
        id: impl Into<String>,
        local: &str,
        ancestor: &str,
        incoming: &str,
        mode: WhitespaceMode,
    ) -> Self {
        Self::open_with(id, local, ancestor, incoming, mode, true)
    }

    /// Like [`open`], but leaves every hunk unresolved instead of auto-applying
    /// one-sided changes — the projection starts as the base text verbatim.
    /// Compare mode uses this so the editable pane opens as the current file and
    /// changes from the ref are only pulled in explicitly.
    ///
    /// [`open`]: Self::open
    pub fn open_unapplied(
        id: impl Into<String>,
        local: &str,
        ancestor: &str,
        incoming: &str,
        mode: WhitespaceMode,
    ) -> Self {
        Self::open_with(id, local, ancestor, incoming, mode, false)
    }

    fn open_with(
        id: impl Into<String>,
        local: &str,
        ancestor: &str,
        incoming: &str,
        mode: WhitespaceMode,
        auto_apply: bool,
    ) -> Self {
        let base = split_lines(ancestor);
        let local = split_lines(local);
        let incoming = split_lines(incoming);
        let regions = diff3(&base, &local, &incoming, mode);

        let mut changes = Vec::new();
        let mut states = Vec::new();
        for (idx, region) in regions.iter().enumerate() {
            if let Region::Change {
                origin, category, ..
            } = region
            {
                changes.push(ChangeMeta {
                    region_idx: idx,
                    origin: *origin,
                    category: *category,
                });
                let init = if *category == Category::Conflicting || !auto_apply {
                    HunkState::Unresolved
                } else {
                    HunkState::Applied {
                        from: natural_side(*origin),
                    }
                };
                states.push(init);
            }
        }

        Self {
            id: id.into(),
            base,
            local,
            incoming,
            mode,
            regions,
            changes,
            states,
            manual: None,
            log: OperationLog::new(),
        }
    }

    pub fn whitespace_mode(&self) -> WhitespaceMode {
        self.mode
    }

    fn chosen_lines(&self, change_id: usize) -> Vec<String> {
        let meta = &self.changes[change_id];
        let region = &self.regions[meta.region_idx];
        let (b, l, r) = match region {
            Region::Change {
                base,
                local,
                incoming,
                ..
            } => (*base, *local, *incoming),
            Region::Stable { .. } => unreachable!(),
        };
        match &self.states[change_id] {
            HunkState::Unresolved | HunkState::Rejected => self.base[b.0..b.1].to_vec(),
            HunkState::Applied { from: Side::Local } => self.local[l.0..l.1].to_vec(),
            HunkState::Applied {
                from: Side::Incoming,
            } => self.incoming[r.0..r.1].to_vec(),
            HunkState::AppliedBoth { first: Side::Local } => {
                let mut v = self.local[l.0..l.1].to_vec();
                v.extend_from_slice(&self.incoming[r.0..r.1]);
                v
            }
            HunkState::AppliedBoth {
                first: Side::Incoming,
            } => {
                let mut v = self.incoming[r.0..r.1].to_vec();
                v.extend_from_slice(&self.local[l.0..l.1]);
                v
            }
            HunkState::ManuallyEdited { lines } => lines.clone(),
        }
    }

    /// Derive the result document, alignment grid, hunk DTOs, and status from the
    /// current hunk states. Pure function of state — the basis for reversibility.
    pub fn to_model(&self) -> SessionModel {
        let mut result: Vec<String> = Vec::new();
        let mut align: Vec<AlignRow> = Vec::new();
        let mut hunks: Vec<ChangeRegion> = Vec::new();
        let mut change_id = 0usize;

        for region in &self.regions {
            match region {
                Region::Stable { local, incoming } => {
                    let n = local.1 - local.0;
                    for k in 0..n {
                        let res_idx = result.len();
                        result.push(self.local[local.0 + k].clone());
                        align.push(AlignRow {
                            local: Some(local.0 + k),
                            result: Some(res_idx),
                            incoming: Some(incoming.0 + k),
                            hunk: None,
                        });
                    }
                }
                Region::Change {
                    base: _,
                    local,
                    incoming,
                    origin,
                    category,
                } => {
                    let id = change_id;
                    change_id += 1;

                    let chosen = self.chosen_lines(id);
                    let res_start = result.len();
                    result.extend(chosen.iter().cloned());
                    let res_end = result.len();

                    let l_len = local.1 - local.0;
                    let r_len = incoming.1 - incoming.0;
                    let h = l_len.max(r_len).max(chosen.len());
                    for k in 0..h {
                        align.push(AlignRow {
                            local: (k < l_len).then(|| local.0 + k),
                            result: (k < chosen.len()).then(|| res_start + k),
                            incoming: (k < r_len).then(|| incoming.0 + k),
                            hunk: Some(id),
                        });
                    }

                    let word_spans = self.word_spans_for(*category, *local, *incoming);

                    hunks.push(ChangeRegion {
                        id,
                        origin: *origin,
                        category: *category,
                        local_range: LineRange::new(local.0, local.1),
                        incoming_range: LineRange::new(incoming.0, incoming.1),
                        result_range: LineRange::new(res_start, res_end),
                        word_spans,
                        state: self.states[id].clone(),
                    });
                }
            }
        }

        SessionModel {
            session_id: self.id.clone(),
            panes: Panes {
                local: self.local.clone(),
                result: self.manual.clone().unwrap_or(result),
                incoming: self.incoming.clone(),
            },
            alignment: align,
            hunks,
            status: self.resolution_status(),
        }
    }

    /// The resolution counters alone — O(#hunks) over the state table, without
    /// building panes, alignment rows, or word spans. List/progress refreshes
    /// poll every open session, so they must not pay `to_model` costs.
    pub fn resolution_status(&self) -> ResolutionStatus {
        let remaining_conflicts = self
            .changes
            .iter()
            .enumerate()
            .filter(|(id, m)| {
                m.category == Category::Conflicting
                    && matches!(self.states[*id], HunkState::Unresolved)
            })
            .count();
        ResolutionStatus {
            total_hunks: self.changes.len(),
            remaining_conflicts,
            fully_resolved: remaining_conflicts == 0,
        }
    }

    fn word_spans_for(
        &self,
        category: Category,
        local: (usize, usize),
        incoming: (usize, usize),
    ) -> Vec<IntraLineSpan> {
        if !matches!(category, Category::Modified | Category::Conflicting) {
            return Vec::new();
        }
        let mut spans = Vec::new();
        let pairs = (local.1 - local.0).min(incoming.1 - incoming.0);
        for k in 0..pairs {
            let a = &self.local[local.0 + k];
            let b = &self.incoming[incoming.0 + k];
            if a == b {
                continue;
            }
            let (a_spans, b_spans) = word_spans(a, b);
            for (s, e) in a_spans {
                spans.push(IntraLineSpan {
                    pane: Pane::Local,
                    row: local.0 + k,
                    start_col: s,
                    end_col: e,
                });
            }
            for (s, e) in b_spans {
                spans.push(IntraLineSpan {
                    pane: Pane::Incoming,
                    row: incoming.0 + k,
                    start_col: s,
                    end_col: e,
                });
            }
        }
        spans
    }

    fn set_state(&mut self, id: usize, next: HunkState) -> Option<HunkChange> {
        let before = self.states.get(id)?.clone();
        if before == next {
            return None;
        }
        self.states[id] = next.clone();
        Some(HunkChange {
            hunk_id: id,
            before,
            after: next,
        })
    }

    /// Record one operation capturing hunk changes plus any manual-override
    /// transition (the gizmo cleared a manual edit), so undo restores both.
    fn commit(&mut self, changes: Vec<HunkChange>, manual_before: Option<Vec<String>>) {
        let manual_after = self.manual.clone();
        let manual = (manual_before != manual_after).then_some(ManualChange {
            before: manual_before,
            after: manual_after,
        });
        self.log.record(Operation { changes, manual });
    }

    pub fn apply(&mut self, id: usize, from: Side) -> SessionModel {
        let manual_before = self.manual.take();
        let changes = self
            .set_state(id, HunkState::Applied { from })
            .into_iter()
            .collect();
        self.commit(changes, manual_before);
        self.to_model()
    }

    /// Keep both sides for one conflict, `first` placed on top (accept-both).
    pub fn apply_both(&mut self, id: usize, first: Side) -> SessionModel {
        let manual_before = self.manual.take();
        let changes = self
            .set_state(id, HunkState::AppliedBoth { first })
            .into_iter()
            .collect();
        self.commit(changes, manual_before);
        self.to_model()
    }

    pub fn revert(&mut self, id: usize) -> SessionModel {
        let manual_before = self.manual.take();
        let changes = self
            .set_state(id, HunkState::Unresolved)
            .into_iter()
            .collect();
        self.commit(changes, manual_before);
        self.to_model()
    }

    /// Apply every non-conflicting change from the requested side(s) in one
    /// reversible operation. `from = None` means both sides (FR-009).
    pub fn apply_non_conflicting(&mut self, from: Option<Side>) -> SessionModel {
        let ids: Vec<(usize, Side)> = self
            .changes
            .iter()
            .enumerate()
            .filter(|(_, m)| m.category != Category::Conflicting)
            .filter_map(|(id, m)| {
                let side = natural_side(m.origin);
                let include = match from {
                    None => true,
                    Some(Side::Local) => m.origin == Origin::Local || m.origin == Origin::Both,
                    Some(Side::Incoming) => {
                        m.origin == Origin::Incoming || m.origin == Origin::Both
                    }
                };
                include.then_some((id, side))
            })
            .collect();

        let manual_before = self.manual.take();
        let mut changes = Vec::new();
        for (id, side) in ids {
            if let Some(c) = self.set_state(id, HunkState::Applied { from: side }) {
                changes.push(c);
            }
        }
        self.commit(changes, manual_before);
        self.to_model()
    }

    /// Record a free-form manual edit of the entire result document. This becomes
    /// the authoritative result until a hunk gizmo operation clears it. Consecutive
    /// edits coalesce into one undo step.
    pub fn set_full_result(&mut self, text: &str) -> SessionModel {
        let manual_before = self.manual.clone();
        self.manual = Some(split_lines(text));
        self.commit(Vec::new(), manual_before);
        self.to_model()
    }

    /// Record a manual edit of the result for the hunk overlapping `[start, end)`.
    pub fn edit_result(&mut self, start: usize, end: usize, text: &str) -> SessionModel {
        let model = self.to_model();
        let target = model.hunks.iter().find(|h| {
            let r = h.result_range;
            start < r.end.max(r.start + 1) && end > r.start
        });
        let target_id = target.map(|h| h.id);
        let manual_before = self.manual.take();
        let mut changes = Vec::new();
        if let Some(id) = target_id {
            let lines = split_lines(text);
            if let Some(change) = self.set_state(id, HunkState::ManuallyEdited { lines }) {
                changes.push(change);
            }
        }
        self.commit(changes, manual_before);
        self.to_model()
    }

    pub fn undo(&mut self) -> SessionModel {
        if let Some(op) = self.log.pop_undo() {
            for c in op.changes.iter().rev() {
                self.states[c.hunk_id] = c.before.clone();
            }
            if let Some(m) = &op.manual {
                self.manual = m.before.clone();
            }
        }
        self.to_model()
    }

    pub fn redo(&mut self) -> SessionModel {
        if let Some(op) = self.log.pop_redo() {
            for c in &op.changes {
                self.states[c.hunk_id] = c.after.clone();
            }
            if let Some(m) = &op.manual {
                self.manual = m.after.clone();
            }
        }
        self.to_model()
    }

    /// Next/prev *unresolved* change id in document order. Resolved changes (drawn
    /// as a dotted ghost outline) are skipped — arrow navigation only stops on
    /// changes still needing a decision. `from = None` starts from the ends.
    /// Returns `None` when no unresolved change remains in that direction.
    pub fn navigate(&self, next: bool, from: Option<usize>) -> Option<usize> {
        let n = self.changes.len();
        if n == 0 {
            return None;
        }
        let unresolved = |i: usize| matches!(self.states[i], HunkState::Unresolved);
        if next {
            (from.map_or(0, |f| f + 1)..n).find(|&i| unresolved(i))
        } else {
            (0..from.unwrap_or(n)).rev().find(|&i| unresolved(i))
        }
    }

    pub fn set_whitespace_mode(&mut self, mode: WhitespaceMode) -> SessionModel {
        *self = MergeSession::open(
            self.id.clone(),
            &self.local.join("\n"),
            &self.base.join("\n"),
            &self.incoming.join("\n"),
            mode,
        );
        self.to_model()
    }
}

fn natural_side(origin: Origin) -> Side {
    match origin {
        Origin::Incoming => Side::Incoming,
        Origin::Local | Origin::Both => Side::Local,
    }
}
