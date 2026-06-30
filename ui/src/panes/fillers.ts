import { EditorView, Decoration, type DecorationSet, WidgetType } from "@codemirror/view";
import { StateField, StateEffect } from "@codemirror/state";
import type { AlignRow, PaneName } from "../ipc/types";

export const setFillers = StateEffect.define<{ pane: PaneName; counts: FillerPlan }>();

// docLineIndex (0-based) -> number of filler rows to insert *before* that line.
// `trailing` filler rows go after the last line.
export interface FillerPlan {
  before: Record<number, number>;
  trailing: number;
}

// Derive, for one pane, how many blank rows precede each real line so the three
// panes stay horizontally aligned (FR-005).
export function planFillers(alignment: AlignRow[], pane: PaneName): FillerPlan {
  const before: Record<number, number> = {};
  let trailing = 0;
  let pending = 0;
  let nextRealLine = 0;
  for (const row of alignment) {
    const idx = pane === "local" ? row.local : pane === "incoming" ? row.incoming : row.result;
    if (idx === null) {
      pending += 1;
    } else {
      if (pending > 0) {
        before[idx] = (before[idx] ?? 0) + pending;
        pending = 0;
      }
      nextRealLine = idx + 1;
    }
  }
  trailing = pending;
  void nextRealLine;
  return { before, trailing };
}

class FillerWidget extends WidgetType {
  constructor(readonly count: number) {
    super();
  }
  eq(other: FillerWidget) {
    return other.count === this.count;
  }
  toDOM(view: EditorView) {
    const el = document.createElement("div");
    el.className = "mcr-filler";
    const h = view.defaultLineHeight * this.count;
    el.style.height = `${h}px`;
    el.setAttribute("aria-hidden", "true");
    return el;
  }
  get estimatedHeight() {
    return -1;
  }
}

function build(view: EditorView, plan: FillerPlan): DecorationSet {
  const decos = [];
  const lines = view.state.doc.lines;
  for (const [idxStr, count] of Object.entries(plan.before)) {
    const idx = Number(idxStr);
    if (idx >= lines) continue;
    const line = view.state.doc.line(idx + 1);
    decos.push(
      Decoration.widget({ widget: new FillerWidget(count), block: true, side: -1 }).range(line.from)
    );
  }
  if (plan.trailing > 0) {
    decos.push(
      Decoration.widget({
        widget: new FillerWidget(plan.trailing),
        block: true,
        side: 1,
      }).range(view.state.doc.length)
    );
  }
  return Decoration.set(
    decos.sort((a, b) => a.from - b.from || (a.value.startSide ?? 0) - (b.value.startSide ?? 0))
  );
}

export function fillerField(pane: PaneName, holder: () => EditorView | undefined) {
  return StateField.define<DecorationSet>({
    create() {
      return Decoration.none;
    },
    update(value, tr) {
      let next = value.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setFillers) && e.value.pane === pane) {
          const view = holder();
          if (view) next = build(view, e.value.counts);
        }
      }
      return next;
    },
    provide: (f) => EditorView.decorations.from(f),
  });
}
