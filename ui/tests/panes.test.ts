import { describe, it, expect } from "vitest";
import { planFillers } from "../src/panes/fillers";
import { MergeEditor } from "../src/panes/merge";
import { demoModel } from "../src/demo";
import type { AlignRow } from "../src/ipc/types";

describe("planFillers (FR-005 alignment)", () => {
  it("inserts filler before the next real line on the short side", () => {
    const alignment: AlignRow[] = [
      { local: 0, result: 0, incoming: 0, hunk: null },
      { local: 1, result: 1, incoming: null, hunk: 1 }, // incoming filler
      { local: 2, result: 2, incoming: 1, hunk: null },
    ];
    const plan = planFillers(alignment, "incoming");
    expect(plan.before[1]).toBe(1); // one filler before incoming line index 1
    expect(plan.trailing).toBe(0);
  });

  it("accumulates trailing filler after the last real line", () => {
    const alignment: AlignRow[] = [
      { local: 0, result: 0, incoming: 0, hunk: null },
      { local: 1, result: 1, incoming: null, hunk: 1 },
      { local: 2, result: 2, incoming: null, hunk: 1 },
    ];
    const plan = planFillers(alignment, "incoming");
    expect(plan.trailing).toBe(2);
  });
});

describe("MergeEditor binding", () => {
  it("loads the model into the three panes without throwing", () => {
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
    expect(editor.local.state.doc.toString()).toBe(model.panes.local.join("\n"));
    expect(editor.result.state.doc.toString()).toBe(model.panes.result.join("\n"));
    expect(editor.incoming.state.doc.toString()).toBe(model.panes.incoming.join("\n"));
  });
});
