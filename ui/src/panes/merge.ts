import { EditorState, Compartment } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import type { PaneName, SessionModel } from "../ipc/types";
import { hunkDecorations, gutterBands, setHunks, viewHolder } from "../highlight/decorations";
import { fillerField, planFillers, setFillers } from "./fillers";
import { editorTheme } from "../theme/editor";
import { themeById } from "../theme/themes";
import { storedThemeId } from "../theme/manager";
import type { ThemePalette } from "../theme/themes";
import { fontExtension, storedFont, type FontSettings } from "../theme/font";
import { syntaxFor } from "../highlight/syntax";

export interface MergeRoots {
  local: HTMLElement;
  result: HTMLElement;
  incoming: HTMLElement;
}

export interface MergeCallbacks {
  // Full result text after a manual edit (free-form editing of the result pane).
  onResultEdit: (fullText: string) => void;
  onGeometryChange: () => void;
}

function makeView(
  parent: HTMLElement,
  pane: PaneName,
  editable: boolean,
  cb: MergeCallbacks,
  langGuard: Compartment,
  themeGuard: Compartment,
  fontGuard: Compartment
): EditorView {
  const editGuard = new Compartment();
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: "",
      extensions: [
        themeGuard.of(editorTheme(themeById(storedThemeId()))),
        fontGuard.of(fontExtension(storedFont())),
        langGuard.of(syntaxFor(null)),
        lineNumbers(),
        hunkDecorations(pane),
        gutterBands(pane),
        fillerField(pane, () => viewHolder.get(pane)),
        editGuard.of([EditorState.readOnly.of(!editable), EditorView.editable.of(editable)]),
        EditorView.updateListener.of((u) => {
          if (u.geometryChanged || u.viewportChanged) cb.onGeometryChange();
          if (editable && u.docChanged && !(u.transactions[0] as any)?.mcrProgrammatic) {
            cb.onResultEdit(u.state.doc.toString());
          }
        }),
        EditorView.theme({
          "&": { height: "100%" },
          // `scrollbar-gutter: stable` reserves the scrollbar track up front so the
          // content width never jumps when the vertical bar appears/disappears
          // (`overflow: overlay` no longer floats reliably in the webview, so the
          // 12px bar was stealing content width only while present).
          ".cm-scroller": {
            overflow: "auto",
            scrollbarGutter: "stable",
          },
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

  private readonly langGuard = new Compartment();
  private readonly themeGuard = new Compartment();
  private readonly fontGuard = new Compartment();

  constructor(roots: MergeRoots, cb: MergeCallbacks) {
    this.local = makeView(roots.local, "local", false, cb, this.langGuard, this.themeGuard, this.fontGuard);
    this.result = makeView(roots.result, "result", true, cb, this.langGuard, this.themeGuard, this.fontGuard);
    this.incoming = makeView(roots.incoming, "incoming", false, cb, this.langGuard, this.themeGuard, this.fontGuard);
  }

  views(): EditorView[] {
    return [this.local, this.result, this.incoming];
  }

  /** Reconfigure all three panes to highlight the given file's language. */
  setLanguage(fileName: string | null | undefined) {
    const ext = syntaxFor(fileName);
    for (const view of this.views()) {
      view.dispatch({ effects: this.langGuard.reconfigure(ext) });
    }
  }

  /** Re-theme all three panes (editor chrome + syntax palette). */
  setTheme(palette: ThemePalette) {
    const ext = editorTheme(palette);
    for (const view of this.views()) {
      view.dispatch({ effects: this.themeGuard.reconfigure(ext) });
    }
  }

  /** Reapply font family, weight, and size to all three panes. */
  setFont(settings: FontSettings) {
    const ext = fontExtension(settings);
    for (const view of this.views()) {
      view.dispatch({ effects: this.fontGuard.reconfigure(ext) });
    }
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
