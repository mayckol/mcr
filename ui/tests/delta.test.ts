import { describe, it, expect } from "vitest";
import { MergeEditor } from "../src/panes/merge";
import { demoModel } from "../src/demo";
import type { SessionModel } from "../src/ipc/types";

describe("incremental model application", () => {
  it("reloads result content when a new model arrives (apply/revert path)", () => {
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
    const { model } = demoModel();
    editor.load(model);

    // Simulate a revert delta: conflict reverts to base, result line 0 changes.
    const reverted: SessionModel = {
      ...model,
      panes: { ...model.panes, result: ["title: BASE", "shared line", "only-local-extra"] },
      hunks: model.hunks.map((h) =>
        h.id === 0 ? { ...h, state: { kind: "unresolved" } as const } : h
      ),
      status: { total_hunks: 2, remaining_conflicts: 1, fully_resolved: false },
    };
    editor.load(reverted);
    expect(editor.result.state.doc.line(1).text).toBe("title: BASE");
  });
});
