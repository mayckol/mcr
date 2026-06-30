import { EditorState, Compartment } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import type { PaneName, SessionModel } from "../ipc/types";
import { hunkDecorations, gutterBands, setHunks, viewHolder } from "../highlight/decorations";
import { fillerField, planFillers, setFillers } from "./fillers";
import { tokyoNight } from "../theme/tokyo";

export interface MergeRoots {
  local: HTMLElement;
  result: HTMLElement;
  incoming: HTMLElement;
}

export interface MergeCallbacks {
  onResultEdit: (fromLine: number, toLine: number, text: string) => void;
  onGeometryChange: () => void;
}

function makeView(
  parent: HTMLElement,
  pane: PaneName,
  editable: boolean,
  cb: MergeCallbacks
): EditorView {
  const editGuard = new Compartment();
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: "",
      extensions: [
        tokyoNight,
        lineNumbers(),
        hunkDecorations(pane),
        gutterBands(pane),
        fillerField(pane, () => viewHolder.get(pane)),
        editGuard.of([EditorState.readOnly.of(!editable), EditorView.editable.of(editable)]),
        EditorView.updateListener.of((u) => {
          if (u.geometryChanged || u.viewportChanged) cb.onGeometryChange();
          if (editable && u.docChanged && !(u.transactions[0] as any)?.mcrProgrammatic) {
            let from = u.state.doc.lines;
            let to = 0;
            u.changes.iterChangedRanges((_fA, _tA, fB, tB) => {
              from = Math.min(from, u.state.doc.lineAt(fB).number - 1);
              to = Math.max(to, u.state.doc.lineAt(tB).number);
            });
            const text = u.state.doc
              .toString()
              .split("\n")
              .slice(from, to)
              .join("\n");
            cb.onResultEdit(from, to, text);
          }
        }),
        EditorView.theme({
          "&": { height: "100%" },
          ".cm-scroller": { overflow: "auto", fontFamily: "ui-monospace, monospace" },
        }),
      ],
    }),
  });
  viewHolder.set(pane, view);
  return view;
}

export class MergeEditor {
  readonly local: EditorView;
  readonly result: EditorView;
  readonly incoming: EditorView;

  constructor(roots: MergeRoots, cb: MergeCallbacks) {
    this.local = makeView(roots.local, "local", false, cb);
    this.result = makeView(roots.result, "result", true, cb);
    this.incoming = makeView(roots.incoming, "incoming", false, cb);
  }

  views(): EditorView[] {
    return [this.local, this.result, this.incoming];
  }

  load(model: SessionModel) {
    this.setDoc(this.local, model.panes.local);
    this.setDoc(this.result, model.panes.result);
    this.setDoc(this.incoming, model.panes.incoming);

    (["local", "result", "incoming"] as PaneName[]).forEach((pane, i) => {
      const view = this.views()[i];
      view.dispatch({
        effects: [
          setHunks.of({ pane, hunks: model.hunks }),
          setFillers.of({ pane, counts: planFillers(model.alignment, pane) }),
        ],
      });
    });
  }

  private setDoc(view: EditorView, lines: string[]) {
    const text = lines.join("\n");
    if (text === view.state.doc.toString()) return;
    const tr = view.state.update({
      changes: { from: 0, to: view.state.doc.length, insert: text },
    });
    (tr as any).mcrProgrammatic = true;
    view.dispatch(tr);
  }
}
