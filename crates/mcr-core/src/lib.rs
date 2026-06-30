//! MCR merge engine: three-way diff, alignment, and reversible apply/revert.
//!
//! All merge logic lives here; the Tauri shell and UI only serialize and render
//! the [`wire::SessionModel`] this crate produces (constitution: Technology Stack).

pub mod diff;
pub mod hunk;
pub mod ops;
pub mod session;
pub mod wire;

pub use diff::WhitespaceMode;
pub use hunk::{Category, ChangeRegion, HunkState, IntraLineSpan, LineRange, Origin, Pane, Side};
pub use session::MergeSession;
pub use wire::{AlignRow, Panes, ResolutionStatus, SessionModel};
