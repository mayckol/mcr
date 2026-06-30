import type { SessionModel } from "./ipc/types";

// Standalone preview data (no backend). In Tauri the real engine replaces this;
// here it lets `vite` render the three-pane UI and drives the frontend tests.
export function demoModel(): {
  localText: string;
  ancestorText: string;
  incomingText: string;
  model: SessionModel;
} {
  const local = ["title: LOCAL", "shared line", "only-local-extra"];
  const ancestor = ["title: BASE", "shared line"];
  const incoming = ["title: INCOMING", "shared line"];
  // Mirror the backend's initial state: the conflict starts UNRESOLVED, so the
  // result shows the base line; the local-only add is auto-applied.
  const result = ["title: BASE", "shared line", "only-local-extra"];

  const model: SessionModel = {
    session_id: "demo",
    panes: { local, result, incoming },
    alignment: [
      { local: 0, result: 0, incoming: 0, hunk: 0 },
      { local: 1, result: 1, incoming: 1, hunk: null },
      { local: 2, result: 2, incoming: null, hunk: 1 },
    ],
    hunks: [
      {
        id: 0,
        origin: "both",
        category: "conflicting",
        local_range: { start: 0, end: 1 },
        incoming_range: { start: 0, end: 1 },
        result_range: { start: 0, end: 1 },
        word_spans: [
          { pane: "local", row: 0, start_col: 7, end_col: 12 },
          { pane: "incoming", row: 0, start_col: 7, end_col: 15 },
        ],
        state: { kind: "unresolved" },
      },
      {
        id: 1,
        origin: "local",
        category: "added",
        local_range: { start: 2, end: 3 },
        incoming_range: { start: 2, end: 2 },
        result_range: { start: 2, end: 3 },
        word_spans: [],
        state: { kind: "applied", from: "local" },
      },
    ],
    status: { total_hunks: 2, remaining_conflicts: 1, fully_resolved: false },
  };

  return {
    localText: local.join("\n"),
    ancestorText: ancestor.join("\n"),
    incomingText: incoming.join("\n"),
    model,
  };
}
