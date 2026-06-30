import { EditorView } from "@codemirror/view";

// Keep the three panes scroll-synchronized (FR-006). Because filler widgets keep
// every pane the same total height, a shared scrollTop aligns rows 1:1.
export function syncScroll(views: EditorView[], onScroll: () => void): () => void {
  let locked = false;
  const handlers: Array<() => void> = [];

  views.forEach((source) => {
    const dom = source.scrollDOM;
    const handler = () => {
      if (locked) return;
      locked = true;
      const top = dom.scrollTop;
      const left = dom.scrollLeft;
      for (const v of views) {
        if (v === source) continue;
        v.scrollDOM.scrollTop = top;
        v.scrollDOM.scrollLeft = left;
      }
      locked = false;
      onScroll();
    };
    dom.addEventListener("scroll", handler, { passive: true });
    handlers.push(() => dom.removeEventListener("scroll", handler));
  });

  return () => handlers.forEach((off) => off());
}
