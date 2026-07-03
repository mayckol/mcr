import { EditorView } from "@codemirror/view";
import type { ChangeRegion, LineRange } from "../ipc/types";
import { CATEGORY_COLORS } from "../highlight/theme";

const SVG_NS = "http://www.w3.org/2000/svg";

interface Edge {
  top: number;
  bottom: number;
}

// Full-line-block vertical client-space extent of a line range, scroll-correct
// and accounting for filler block widgets; null if off-screen (cull — SC-004/007).
function edge(view: EditorView, range: LineRange): Edge | null {
  const doc = view.state.doc;
  if (doc.lines === 0) return null;
  const startLine = Math.min(range.start, doc.lines - 1);
  const endLine = Math.min(Math.max(range.end - 1, range.start), doc.lines - 1);
  let top: number;
  let bottom: number;
  try {
    const bs = view.lineBlockAt(doc.line(startLine + 1).from);
    const be = view.lineBlockAt(doc.line(endLine + 1).from);
    top = view.documentTop + bs.top;
    bottom = view.documentTop + be.bottom;
  } catch {
    return null;
  }
  const pr = view.scrollDOM.getBoundingClientRect();
  if (bottom <= pr.top || top >= pr.bottom) return null; // fully off-screen
  return { top: Math.max(top, pr.top), bottom: Math.min(bottom, pr.bottom) };
}

// SVG overlay that binds each side hunk to its aligned result region with a
// curved band, re-projected from live editor geometry (FR-004, FR-006, R3).
export class ConnectorOverlay {
  private svg: SVGSVGElement;
  constructor(
    private container: HTMLElement,
    private left: EditorView,
    private result: EditorView,
    private right: EditorView
  ) {
    this.svg = document.createElementNS(SVG_NS, "svg");
    this.svg.classList.add("mcr-connectors");
    Object.assign(this.svg.style, {
      position: "absolute",
      inset: "0",
      width: "100%",
      height: "100%",
      pointerEvents: "none",
      zIndex: "5",
    });
    container.appendChild(this.svg);
  }

  render(hunks: ChangeRegion[]) {
    const cr = this.container.getBoundingClientRect();
    const lr = this.left.scrollDOM.getBoundingClientRect();
    const rr = this.result.scrollDOM.getBoundingClientRect();
    const ir = this.right.scrollDOM.getBoundingClientRect();

    const xLeftGutter = lr.right - cr.left;
    const xResultLeft = rr.left - cr.left;
    const xResultRight = rr.right - cr.left;
    const xRightGutter = ir.left - cr.left;

    while (this.svg.firstChild) this.svg.removeChild(this.svg.firstChild);

    for (const h of hunks) {
      const res = edge(this.result, h.result_range);
      if (!res) continue;
      const strong = h.state.kind === "unresolved";

      if (h.origin === "local" || h.origin === "both" || h.category === "conflicting") {
        const le = edge(this.left, h.local_range);
        if (le) {
          this.band(xLeftGutter, le, xResultLeft, res, h.category, cr.top, strong);
        }
      }
      if (h.origin === "incoming" || h.origin === "both" || h.category === "conflicting") {
        const re = edge(this.right, h.incoming_range);
        if (re) {
          this.band(xResultRight, res, xRightGutter, re, h.category, cr.top, strong);
        }
      }
    }
  }

  private band(
    x1: number,
    e1: Edge,
    x2: number,
    e2: Edge,
    cat: ChangeRegion["category"],
    offY: number,
    strong: boolean
  ) {
    const fill = CATEGORY_COLORS[cat].band;
    const stroke = CATEGORY_COLORS[cat].connector;
    // Ribbon stays flush within the gutter gap — overlapping into the editor
    // would draw a stray vertical bar over the content.
    const xa = x1;
    const xb = x2;
    const y1t = e1.top - offY;
    const y1b = e1.bottom - offY;
    const y2t = e2.top - offY;
    const y2b = e2.bottom - offY;
    const cx = (xa + xb) / 2;
    const path = document.createElementNS(SVG_NS, "path");
    const d = [
      `M ${xa} ${y1t}`,
      `C ${cx} ${y1t}, ${cx} ${y2t}, ${xb} ${y2t}`,
      `L ${xb} ${y2b}`,
      `C ${cx} ${y2b}, ${cx} ${y1b}, ${xa} ${y1b}`,
      "Z",
    ].join(" ");
    void stroke;
    path.setAttribute("d", d);
    // Style properties, not presentation attributes: the fill is a CSS var()
    // reference (theme-driven), which attributes don't resolve.
    path.style.fill = fill;
    path.style.fillOpacity = strong ? "1" : "0.6";
    path.style.stroke = "none";
    this.svg.appendChild(path);
  }

  destroy() {
    this.svg.remove();
  }
}
