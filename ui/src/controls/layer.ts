import { EditorView } from "@codemirror/view";
import type { ChangeRegion } from "../ipc/types";

export interface ControlCallbacks {
  applyLocal: (hunkId: number) => void;
  applyIncoming: (hunkId: number) => void;
  revert: (hunkId: number) => void;
}

function midY(view: EditorView, startLine: number, endLine: number, offY: number): number | null {
  const doc = view.state.doc;
  if (doc.lines === 0) return null;
  const s = Math.min(startLine, doc.lines - 1);
  const e = Math.min(Math.max(endLine - 1, startLine), doc.lines - 1);
  let top: number;
  let bottom: number;
  try {
    const bs = view.lineBlockAt(doc.line(s + 1).from);
    const be = view.lineBlockAt(doc.line(e + 1).from);
    top = view.documentTop + bs.top;
    bottom = view.documentTop + be.bottom;
  } catch {
    return null;
  }
  const pr = view.scrollDOM.getBoundingClientRect();
  if (bottom <= pr.top || top >= pr.bottom) return null;
  return (top + bottom) / 2 - offY;
}

// Clickable apply/revert affordances anchored to live hunk geometry (FR-007/008).
export class ControlsLayer {
  private host: HTMLElement;
  constructor(
    private container: HTMLElement,
    private left: EditorView,
    private result: EditorView,
    private right: EditorView,
    private cb: ControlCallbacks
  ) {
    this.host = document.createElement("div");
    Object.assign(this.host.style, { position: "absolute", inset: "0", pointerEvents: "none", zIndex: "6" });
    container.appendChild(this.host);
  }

  private button(label: string, x: number, y: number, title: string, onClick: () => void) {
    const btn = document.createElement("button");
    btn.textContent = label;
    btn.title = title;
    btn.className = "mcr-gizmo";
    Object.assign(btn.style, {
      position: "absolute",
      left: `${x}px`,
      top: `${y - 9}px`,
      pointerEvents: "auto",
    });
    btn.addEventListener("click", (e) => {
      e.preventDefault();
      onClick();
    });
    this.host.appendChild(btn);
  }

  render(hunks: ChangeRegion[]) {
    const cr = this.container.getBoundingClientRect();
    const lr = this.left.scrollDOM.getBoundingClientRect();
    const rr = this.result.scrollDOM.getBoundingClientRect();
    const ir = this.right.scrollDOM.getBoundingClientRect();
    while (this.host.firstChild) this.host.removeChild(this.host.firstChild);

    for (const h of hunks) {
      const applied = h.state.kind === "applied";
      const showLocal = h.origin === "local" || h.origin === "both" || h.category === "conflicting";
      const showIncoming =
        h.origin === "incoming" || h.origin === "both" || h.category === "conflicting";

      if (showLocal) {
        const y = midY(this.left, h.local_range.start, h.local_range.end, cr.top);
        if (y !== null) this.button("»", lr.right - cr.left - 18, y, "Apply from left", () => this.cb.applyLocal(h.id));
      }
      if (showIncoming) {
        const y = midY(this.right, h.incoming_range.start, h.incoming_range.end, cr.top);
        if (y !== null) this.button("«", ir.left - cr.left + 2, y, "Apply from right", () => this.cb.applyIncoming(h.id));
      }
      if (applied) {
        const y = midY(this.result, h.result_range.start, h.result_range.end, cr.top);
        if (y !== null) this.button("×", rr.right - cr.left - 18, y, "Revert this change", () => this.cb.revert(h.id));
      }
    }
  }

  destroy() {
    this.host.remove();
  }
}
