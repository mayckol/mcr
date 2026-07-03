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

    // Each pane reserves a scrollbar rail its line backgrounds can't paint into,
    // so a band stops a bar-width short of the pane edge. The bar sits on the
    // OUTER edge of every pane (local's on the left via the rtl scroller, result's
    // and incoming's on the right). We anchor the ribbons at the pane's inner
    // CONTENT edge and separately cap the outer rail with the band tint, so the
    // highlight reaches the pane edge with the scrollbar sitting within it.
    const bar = (v: EditorView) => v.scrollDOM.offsetWidth - v.scrollDOM.clientWidth;
    const barLeft = bar(this.left);
    const barResult = bar(this.result);
    const barRight = bar(this.right);
    const xLeftGutter = lr.right - cr.left;
    const xResultLeft = rr.left - cr.left;
    const xResultRight = rr.right - cr.left - barResult;
    const xRightGutter = ir.left - cr.left;

    // Cap only a pane's OUTER edge — the side with no connectors. Local's bar is
    // always outer-left; incoming's is outer-right. The result pane's bar is on its
    // right, which is an INNER (connector) edge in the 3-pane merge — there the
    // ribbons already bridge the band to incoming, so a cap would only stack over
    // them and muddy the tint. In 2-pane compare the result IS the rightmost pane,
    // so its right edge is outer and does need the cap.
    const compare = this.container.classList.contains("two-pane");

    while (this.svg.firstChild) this.svg.removeChild(this.svg.firstChild);

    for (const h of hunks) {
      const strong = h.state.kind === "unresolved";
      const conflict = h.category === "conflicting";
      const hasLocal = conflict || h.origin === "local" || h.origin === "both";
      const hasIncoming = conflict || h.origin === "incoming" || h.origin === "both";

      // A resolved change carries no fill and no ribbon — a dotted outline marks
      // where each side's band used to be, so the accepted region reads as a ghost.
      if (!strong) {
        // Span the full pane width (scroller edge to edge, scrollbar rail included)
        // so the ghost matches the filled band's reach rather than stopping short.
        if (hasLocal) this.outline(this.left, h.local_range, lr.left - cr.left, lr.right - cr.left, h.category, cr.top);
        this.outline(this.result, h.result_range, rr.left - cr.left, rr.right - cr.left, h.category, cr.top);
        if (hasIncoming) this.outline(this.right, h.incoming_range, ir.left - cr.left, ir.right - cr.left, h.category, cr.top);
        continue;
      }

      // Outer-rail caps first, so the connector ribbons paint over them. Cap a side
      // only when it is banded (it participated in the change) — an unbanded side
      // would otherwise get a stray tint in its scrollbar rail.
      if (hasLocal) {
        this.cap(this.left, h.local_range, lr.left - cr.left, barLeft, h.category, cr.top, strong);
      }
      if (compare) {
        this.cap(this.result, h.result_range, rr.right - cr.left - barResult, barResult, h.category, cr.top, strong);
      }
      if (hasIncoming) {
        this.cap(this.right, h.incoming_range, ir.right - cr.left - barRight, barRight, h.category, cr.top, strong);
      }

      const res = edge(this.result, h.result_range);
      if (!res) continue;

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

  // Dotted outline around a resolved hunk's line range in one pane — the ghost of
  // a change that no longer carries a fill. No-op for an empty range (e.g. the
  // deleted side of a removal) or when the range is off-screen.
  private outline(
    view: EditorView,
    range: LineRange,
    xLeft: number,
    xRight: number,
    cat: ChangeRegion["category"],
    offY: number
  ) {
    if (range.end <= range.start || xRight <= xLeft) return;
    const e = edge(view, range);
    if (!e) return;
    const rect = document.createElementNS(SVG_NS, "rect");
    rect.setAttribute("x", String(xLeft + 0.5));
    rect.setAttribute("y", String(e.top - offY + 0.5));
    rect.setAttribute("width", String(xRight - xLeft - 1));
    rect.setAttribute("height", String(e.bottom - e.top - 1));
    rect.setAttribute("rx", "3");
    rect.style.fill = "none";
    rect.style.stroke = CATEGORY_COLORS[cat].connector;
    rect.style.strokeWidth = "1";
    rect.style.strokeDasharray = "2 3";
    rect.style.strokeOpacity = "0.6";
    this.svg.appendChild(rect);
  }

  // Fill a pane's reserved scrollbar rail with the band tint across a hunk's line
  // range, so the change highlight reaches the pane edge instead of stopping at the
  // content edge. No-op when the rail is zero-width (no scrollbar) or off-screen.
  private cap(
    view: EditorView,
    range: LineRange,
    x: number,
    width: number,
    cat: ChangeRegion["category"],
    offY: number,
    strong: boolean
  ) {
    if (width <= 0 || range.end <= range.start) return;
    const e = edge(view, range);
    if (!e) return;
    const rect = document.createElementNS(SVG_NS, "rect");
    rect.setAttribute("x", String(x));
    rect.setAttribute("y", String(e.top - offY));
    rect.setAttribute("width", String(width));
    rect.setAttribute("height", String(e.bottom - e.top));
    rect.style.fill = CATEGORY_COLORS[cat].band;
    if (!strong) rect.style.opacity = "0.7";
    this.svg.appendChild(rect);
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
