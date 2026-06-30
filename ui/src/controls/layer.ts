import { EditorView } from "@codemirror/view";
import type { ChangeRegion, Side } from "../ipc/types";

export interface ControlCallbacks {
  applyLocal: (hunkId: number) => void;
  applyIncoming: (hunkId: number) => void;
  // Keep both sides for a conflict; `first` is the side placed on top.
  acceptBoth: (hunkId: number, first: Side) => void;
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
      top: `${y - 10}px`,
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
      const isConflict = h.category === "conflicting";
      const appliedFrom = h.state.kind === "applied" ? h.state.from : null;
      const showRevert = h.state.kind === "applied" || h.state.kind === "applied_both";
      const showLocal = h.origin === "local" || h.origin === "both" || isConflict;
      const showIncoming = h.origin === "incoming" || h.origin === "both" || isConflict;

      if (showLocal) {
        const y = midY(this.left, h.local_range.start, h.local_range.end, cr.top);
        const x = lr.right - cr.left - 24;
        // Incoming already applied → left becomes "append local after right".
        if (y !== null && isConflict && appliedFrom === "incoming") {
          this.button("»+", x, y, "Append left after right", () => this.cb.acceptBoth(h.id, "incoming"));
        } else if (y !== null) {
          this.button("»", x, y, "Apply from left", () => this.cb.applyLocal(h.id));
        }
      }
      if (showIncoming) {
        const y = midY(this.right, h.incoming_range.start, h.incoming_range.end, cr.top);
        const x = ir.left - cr.left + 2;
        // Local already applied → right becomes "append right after left".
        if (y !== null && isConflict && appliedFrom === "local") {
          this.button("«+", x, y, "Append right after left", () => this.cb.acceptBoth(h.id, "local"));
        } else if (y !== null) {
          this.button("«", x, y, "Apply from right", () => this.cb.applyIncoming(h.id));
        }
      }
      if (showRevert) {
        const y = midY(this.result, h.result_range.start, h.result_range.end, cr.top);
        if (y !== null) this.button("×", rr.right - cr.left - 24, y, "Revert this change", () => this.cb.revert(h.id));
      }
    }
  }

  destroy() {
    this.host.remove();
  }
}
