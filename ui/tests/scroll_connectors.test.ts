import { describe, it, expect } from "vitest";
import { MergeEditor } from "../src/panes/merge";
import { ConnectorOverlay } from "../src/connectors/overlay";
import { demoModel } from "../src/demo";

describe("ConnectorOverlay (FR-004/006)", () => {
  it("renders against live geometry without throwing (off-screen rows cull)", () => {
    document.body.innerHTML =
      '<div id="c"><div id="l"></div><div id="r"></div><div id="i"></div></div>';
    const container = document.getElementById("c")!;
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
    const overlay = new ConnectorOverlay(container, editor.local, editor.result, editor.incoming);
    // In jsdom coordsAtPos is null, so every connector is culled — must not crash.
    expect(() => overlay.render(model.hunks)).not.toThrow();
    expect(container.querySelector("svg.mcr-connectors")).toBeTruthy();
  });
});
