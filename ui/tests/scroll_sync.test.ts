import { describe, it, expect } from "vitest";
import type { EditorView } from "@codemirror/view";
import { syncScroll } from "../src/scroll/sync";

// Minimal stand-in: syncScroll only ever touches `view.scrollDOM`.
function fakeView(): EditorView {
  return { scrollDOM: document.createElement("div") } as unknown as EditorView;
}

function scroll(view: EditorView, pos: { top?: number; left?: number }) {
  if (pos.top !== undefined) view.scrollDOM.scrollTop = pos.top;
  if (pos.left !== undefined) view.scrollDOM.scrollLeft = pos.left;
  view.scrollDOM.dispatchEvent(new Event("scroll"));
}

const tops = (vs: EditorView[]) => vs.map((v) => v.scrollDOM.scrollTop);
const lefts = (vs: EditorView[]) => vs.map((v) => v.scrollDOM.scrollLeft);

describe("syncScroll (FR-006)", () => {
  it("mirrors vertical scroll from any pane to all three", () => {
    const views = [fakeView(), fakeView(), fakeView()]; // [local, result, incoming]
    const off = syncScroll(views, views[1], () => {});

    scroll(views[0], { top: 30 }); // a side pane drives
    expect(tops(views)).toEqual([30, 30, 30]);

    scroll(views[2], { top: 75 }); // the other side drives
    expect(tops(views)).toEqual([75, 75, 75]);
    off();
  });

  it("lets the middle (result) pane drive horizontal scroll for all three", () => {
    const views = [fakeView(), fakeView(), fakeView()];
    const off = syncScroll(views, views[1], () => {});

    scroll(views[1], { left: 120 });
    expect(lefts(views)).toEqual([120, 120, 120]);
    off();
  });

  it("keeps side panes' horizontal scroll independent", () => {
    const views = [fakeView(), fakeView(), fakeView()];
    const off = syncScroll(views, views[1], () => {});

    scroll(views[0], { left: 200 }); // LOCAL pans alone
    expect(lefts(views)).toEqual([200, 0, 0]);

    scroll(views[2], { left: 90 }); // INCOMING pans alone
    expect(lefts(views)).toEqual([200, 0, 90]);
    off();
  });

  it("invokes the onScroll callback for connector re-anchoring", () => {
    const views = [fakeView(), fakeView(), fakeView()];
    let calls = 0;
    const off = syncScroll(views, views[1], () => calls++);

    scroll(views[0], { left: 10 }); // even an independent side scroll re-anchors
    expect(calls).toBeGreaterThan(0);
    off();
  });
});
