import { EditorView } from "@codemirror/view";

// Keep the three panes scroll-synchronized (FR-006).
//
// Vertical: any pane drives the other two. Filler widgets keep every pane the
// same total height, so mirroring scrollTop aligns rows 1:1 — even when one side
// has more real lines, its fillers pad the difference so the pixel offset maps to
// the same row everywhere.
//
// Horizontal: only the `master` (the middle / RESULT pane) drives all three.
// Scrolling RESULT left/right pans every pane in lockstep so the resolved text
// stays column-aligned with both sides. The side panes (LOCAL / INCOMING) own
// their own long lines and pan independently — sliding a side never disturbs the
// others. The browser clamps an over-large offset to each pane's own range, so a
// shorter pane never desyncs.
export function syncScroll(
  views: EditorView[],
  master: EditorView,
  onScroll: () => void
): () => void {
  let locked = false;
  const handlers: Array<() => void> = [];

  views.forEach((source) => {
    const dom = source.scrollDOM;
    const drivesHorizontal = source === master;
    let lastTop = dom.scrollTop;
    let lastLeft = dom.scrollLeft;
    const handler = () => {
      if (locked) return;
      const top = dom.scrollTop;
      const left = dom.scrollLeft;
      const vChanged = top !== lastTop;
      const hChanged = drivesHorizontal && left !== lastLeft;
      if (vChanged || hChanged) {
        locked = true;
        for (const v of views) {
          if (v === source) continue;
          if (vChanged) v.scrollDOM.scrollTop = top;
          if (hChanged) v.scrollDOM.scrollLeft = left;
        }
        locked = false;
      }
      lastTop = top;
      lastLeft = left;
      onScroll();
    };
    dom.addEventListener("scroll", handler, { passive: true });
    handlers.push(() => dom.removeEventListener("scroll", handler));
  });

  return () => handlers.forEach((off) => off());
}
