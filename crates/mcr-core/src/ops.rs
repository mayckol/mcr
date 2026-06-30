use crate::hunk::HunkState;

/// A reversible operation: one or more hunk state transitions captured with
/// their before/after so undo/redo restore the exact prior state (FR-010, SC-005).
#[derive(Clone, Debug)]
pub struct Operation {
    pub changes: Vec<HunkChange>,
}

#[derive(Clone, Debug)]
pub struct HunkChange {
    pub hunk_id: usize,
    pub before: HunkState,
    pub after: HunkState,
}

#[derive(Default)]
pub struct OperationLog {
    undo: Vec<Operation>,
    redo: Vec<Operation>,
}

impl OperationLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a freshly-applied operation; clears the redo stack.
    pub fn record(&mut self, op: Operation) {
        if op.changes.is_empty() {
            return;
        }
        self.undo.push(op);
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Pop the last operation for undo, moving it onto the redo stack.
    pub fn pop_undo(&mut self) -> Option<Operation> {
        let op = self.undo.pop()?;
        self.redo.push(op.clone());
        Some(op)
    }

    /// Pop the last undone operation for redo, moving it back onto the undo stack.
    pub fn pop_redo(&mut self) -> Option<Operation> {
        let op = self.redo.pop()?;
        self.undo.push(op.clone());
        Some(op)
    }
}
