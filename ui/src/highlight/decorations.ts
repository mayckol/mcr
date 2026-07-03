import {
  EditorView,
  Decoration,
  type DecorationSet,
  gutterLineClass,
  GutterMarker,
} from "@codemirror/view";
import { StateField, StateEffect, RangeSetBuilder, RangeSet, type Range } from "@codemirror/state";
import type { ChangeRegion, PaneName } from "../ipc/types";
import { CATEGORY_COLORS } from "./theme";

export const setHunks = StateEffect.define<{ pane: PaneName; hunks: ChangeRegion[] }>();

function rangeFor(h: ChangeRegion, pane: PaneName): { start: number; end: number } {
  if (pane === "local") return h.local_range;
  if (pane === "incoming") return h.incoming_range;
  return h.result_range;
}

function isResolved(h: ChangeRegion): boolean {
  return h.state.kind !== "unresolved";
}

// Whether a side pane took part in the change. The backend anchors every hunk in
// all three panes (an incoming-only edit still carries the matching, unchanged
// local lines), but banding the side that did NOT change leaves a highlight with
// no connector ribbon — it reads as broken. Band a side only when it participated,
// matching the connector/control gating; the result pane always shows the change.
function participates(h: ChangeRegion, pane: PaneName): boolean {
  if (pane === "result" || h.category === "conflicting") return true;
  if (pane === "local") return h.origin === "local" || h.origin === "both";
  return h.origin === "incoming" || h.origin === "both";
}

// Build line-band + word-span decorations for one pane from the hunk list.
// Decorations are collected then sorted by Decoration.set: line-bands (at a line's
// start) and word-marks (mid-line) interleave across positions, so a pre-sorted
// RangeSetBuilder would throw on any multi-line hunk that also has word spans.
function build(view: EditorView, pane: PaneName, hunks: ChangeRegion[]): DecorationSet {
  const docLines = view.state.doc.lines;
  const decos: Range<Decoration>[] = [];

  for (const h of hunks) {
    if (!participates(h, pane)) continue;
    const { start, end } = rangeFor(h, pane);
    const colors = CATEGORY_COLORS[h.category];
    const dim = isResolved(h) ? " mcr-resolved" : "";
    for (let ln = start; ln < end && ln < docLines; ln++) {
      const line = view.state.doc.line(ln + 1);
      decos.push(
        Decoration.line({
          attributes: {
            class: `mcr-band mcr-${h.category}${dim}`,
            style: `background:${colors.band}`,
          },
        }).range(line.from)
      );
    }
    for (const span of h.word_spans) {
      if (span.pane !== pane) continue;
      if (span.row >= docLines) continue;
      const line = view.state.doc.line(span.row + 1);
      const from = line.from + Math.min(span.start_col, line.length);
      const to = line.from + Math.min(span.end_col, line.length);
      if (to > from) {
        decos.push(
          Decoration.mark({
            class: `mcr-word mcr-${h.category}`,
            attributes: { style: `background:${colors.word}` },
          }).range(from, to)
        );
      }
    }
  }
  return Decoration.set(decos, true);
}

// Gutter line marker so the line-number column shares the change tint
// (the band overlaps the editor's line column, per the reference UI).
class BandGutterMarker extends GutterMarker {
  elementClass: string;
  constructor(cls: string) {
    super();
    this.elementClass = cls;
  }
  eq(other: BandGutterMarker) {
    return other.elementClass === this.elementClass;
  }
}

function buildGutter(view: EditorView, pane: PaneName, hunks: ChangeRegion[]): RangeSet<GutterMarker> {
  const builder = new RangeSetBuilder<GutterMarker>();
  const docLines = view.state.doc.lines;
  const ordered = [...hunks].sort((a, b) => rangeFor(a, pane).start - rangeFor(b, pane).start);
  for (const h of ordered) {
    if (!participates(h, pane)) continue;
    const { start, end } = rangeFor(h, pane);
    const dim = isResolved(h) ? " mcr-g-resolved" : "";
    for (let ln = start; ln < end && ln < docLines; ln++) {
      const line = view.state.doc.line(ln + 1);
      builder.add(line.from, line.from, new BandGutterMarker(`mcr-g mcr-g-${h.category}${dim}`));
    }
  }
  return builder.finish();
}

export function hunkDecorations(pane: PaneName) {
  return StateField.define<DecorationSet>({
    create() {
      return Decoration.none;
    },
    update(value, tr) {
      let next = value.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setHunks) && e.value.pane === pane) {
          next = build(viewHolder.get(pane)!, pane, e.value.hunks);
        }
      }
      return next;
    },
    provide: (f) => EditorView.decorations.from(f),
  });
}

export function gutterBands(pane: PaneName) {
  return StateField.define<RangeSet<GutterMarker>>({
    create() {
      return RangeSet.empty;
    },
    update(value, tr) {
      let next = value.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setHunks) && e.value.pane === pane) {
          next = buildGutter(viewHolder.get(pane)!, pane, e.value.hunks);
        }
      }
      return next;
    },
    provide: (f) => gutterLineClass.from(f),
  });
}

class ViewHolder {
  private views = new Map<PaneName, EditorView>();
  set(pane: PaneName, view: EditorView) {
    this.views.set(pane, view);
  }
  get(pane: PaneName) {
    return this.views.get(pane);
  }
}
export const viewHolder = new ViewHolder();
