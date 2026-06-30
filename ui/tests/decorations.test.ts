import { describe, it, expect } from "vitest";
import { MergeEditor } from "../src/panes/merge";
import type { SessionModel, ChangeRegion } from "../src/ipc/types";

// A multi-line conflict whose word spans fall on non-adjacent rows: before the
// fix this made the decoration builder receive out-of-order ranges and throw,
// dropping every band (the bug where the middle pane showed no highlight).
function multiLineConflictModel(): SessionModel {
  const lines = ["x", "c line 62", "c line 63", "c line 64", "y"];
  const hunk: ChangeRegion = {
    id: 0,
    origin: "incoming",
    category: "conflicting",
    local_range: { start: 1, end: 4 },
    incoming_range: { start: 1, end: 4 },
    result_range: { start: 1, end: 4 },
    word_spans: [
      { pane: "result", row: 1, start_col: 0, end_col: 6 },
      { pane: "result", row: 3, start_col: 0, end_col: 6 },
      { pane: "local", row: 1, start_col: 0, end_col: 6 },
      { pane: "local", row: 3, start_col: 0, end_col: 6 },
    ],
    state: { kind: "unresolved" },
  };
  return {
    session_id: "t",
    panes: { local: lines, result: lines, incoming: lines },
    alignment: lines.map((_, i) => ({ local: i, result: i, incoming: i, hunk: i >= 1 && i <= 3 ? 0 : null })),
    hunks: [hunk],
    status: { total_hunks: 1, remaining_conflicts: 1, fully_resolved: false },
  };
}

describe("decorations build (multi-line conflict)", () => {
  it("loads a multi-line conflict with interleaving word spans without throwing", () => {
    document.body.innerHTML =
      '<div id="c"><div id="l"></div><div id="r"></div><div id="i"></div></div>';
    const editor = new MergeEditor(
      {
        local: document.getElementById("l")!,
        result: document.getElementById("r")!,
        incoming: document.getElementById("i")!,
      },
      { onResultEdit: () => {}, onGeometryChange: () => {} }
    );
    expect(() => editor.load(multiLineConflictModel())).not.toThrow();
    // The result pane holds the conflict's base lines that must carry the band.
    expect(editor.result.state.doc.toString()).toContain("c line 63");
  });
});
