// Wire types mirroring crates/mcr-core/src/wire.rs and hunk.rs. Keep in sync.

export type Side = "local" | "incoming";
export type Origin = "local" | "incoming" | "both";
export type Category = "added" | "removed" | "modified" | "conflicting";
export type PaneName = "local" | "result" | "incoming";

export type HunkState =
  | { kind: "unresolved" }
  | { kind: "applied"; from: Side }
  | { kind: "rejected" }
  | { kind: "manually_edited"; lines: string[] };

export interface LineRange {
  start: number;
  end: number;
}

export interface IntraLineSpan {
  pane: PaneName;
  row: number;
  start_col: number;
  end_col: number;
}

export interface ChangeRegion {
  id: number;
  origin: Origin;
  category: Category;
  local_range: LineRange;
  incoming_range: LineRange;
  result_range: LineRange;
  word_spans: IntraLineSpan[];
  state: HunkState;
}

export interface AlignRow {
  local: number | null;
  result: number | null;
  incoming: number | null;
  hunk: number | null;
}

export interface Panes {
  local: string[];
  result: string[];
  incoming: string[];
}

export interface ResolutionStatus {
  total_hunks: number;
  remaining_conflicts: number;
  fully_resolved: boolean;
}

export interface SessionModel {
  session_id: string;
  panes: Panes;
  alignment: AlignRow[];
  hunks: ChangeRegion[];
  status: ResolutionStatus;
}
