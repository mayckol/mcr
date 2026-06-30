import { MergeEditor } from "./panes/merge";
import { ConnectorOverlay } from "./connectors/overlay";
import { ControlsLayer } from "./controls/layer";
import { syncScroll } from "./scroll/sync";
import { ipc } from "./ipc/client";
import { demoModel } from "./demo";
import { Shortcuts } from "./shortcuts/keymap";
import { ShortcutsPanel } from "./shortcuts/panel";
import type { SessionModel } from "./ipc/types";

const inTauri = "__TAURI_INTERNALS__" in window || "__TAURI__" in window;

const $ = (id: string) => document.getElementById(id)!;

let model: SessionModel;
let currentHunk: number | null = null;

const merge = new MergeEditor(
  { local: $("pane-local"), result: $("pane-result"), incoming: $("pane-incoming") },
  {
    onResultEdit: async (from, to, text) => {
      if (!inTauri) return;
      apply(await ipc.editResult(model.session_id, from, to, text));
    },
    onGeometryChange: () => scheduleRefresh(),
  }
);

const container = $("merge-container");
const overlay = new ConnectorOverlay(container, merge.local, merge.result, merge.incoming);
const controls = new ControlsLayer(container, merge.local, merge.result, merge.incoming, {
  applyLocal: (id) => act(() => mutate((s) => ipc.applyChange(s, id, "local"))),
  applyIncoming: (id) => act(() => mutate((s) => ipc.applyChange(s, id, "incoming"))),
  revert: (id) => act(() => mutate((s) => ipc.revertChange(s, id))),
});

let rafPending = false;
function scheduleRefresh() {
  if (rafPending) return;
  rafPending = true;
  requestAnimationFrame(() => {
    rafPending = false;
    overlay.render(model.hunks);
    controls.render(model.hunks);
  });
}

function apply(next: SessionModel) {
  model = next;
  merge.load(model);
  renderStatus();
  scheduleRefresh();
  // The webview may finish font/layout measurement a frame or two later; re-anchor
  // connectors then so they never freeze at a stale (pre-layout) position.
  setTimeout(scheduleRefresh, 60);
  setTimeout(scheduleRefresh, 250);
}

async function mutate(fn: (sessionId: string) => Promise<SessionModel>): Promise<SessionModel> {
  if (!inTauri) return model; // demo mode: backend mutations are no-ops
  return fn(model.session_id);
}

function act(fn: () => Promise<SessionModel>) {
  fn().then(apply);
}

function renderStatus() {
  const s = model.status;
  $("status").textContent = s.fully_resolved
    ? `Resolved — ${s.total_hunks} changes`
    : `${s.remaining_conflicts} conflict(s) remaining of ${s.total_hunks} changes`;
}

// Shared action handlers — used by both the toolbar and the shortcut keymap.
const actions = {
  undo: () => act(() => mutate((s) => ipc.undo(s))),
  redo: () => act(() => mutate((s) => ipc.redo(s))),
  applyAll: () => act(() => mutate((s) => ipc.applyNonConflicting(s, "both"))),
  applyLeft: () => act(() => mutate((s) => ipc.applyNonConflicting(s, "local"))),
  applyRight: () => act(() => mutate((s) => ipc.applyNonConflicting(s, "incoming"))),
  next: () => navigate("next"),
  prev: () => navigate("prev"),
};

// Toolbar
$("apply-left").addEventListener("click", actions.applyLeft);
$("apply-right").addEventListener("click", actions.applyRight);
$("apply-all").addEventListener("click", actions.applyAll);
$("undo").addEventListener("click", actions.undo);
$("redo").addEventListener("click", actions.redo);
$("prev").addEventListener("click", actions.prev);
$("next").addEventListener("click", actions.next);
$("whitespace").addEventListener("change", (e) => {
  const mode = (e.target as HTMLSelectElement).value as "none" | "ignore_trailing" | "ignore_all";
  act(() => mutate((s) => ipc.setWhitespaceMode(s, mode)));
});

// Configurable keyboard shortcuts (Cmd+Z / Cmd+Shift+Z by default).
const shortcuts = new Shortcuts();
shortcuts.on(actions);
shortcuts.attach(window);
const shortcutsPanel = new ShortcutsPanel(shortcuts);
$("shortcuts").addEventListener("click", () => shortcutsPanel.open());

async function navigate(direction: "next" | "prev") {
  if (!inTauri) return;
  const id = await ipc.navigate(model.session_id, direction, currentHunk);
  currentHunk = id;
  if (id === null) return;
  const h = model.hunks.find((x) => x.id === id);
  if (!h) return;
  const line = merge.result.state.doc.line(Math.min(h.result_range.start + 1, merge.result.state.doc.lines));
  merge.result.dispatch({ selection: { anchor: line.from }, scrollIntoView: true });
}

syncScroll(merge.views(), merge.result, scheduleRefresh);
new ResizeObserver(scheduleRefresh).observe(container);
window.addEventListener("resize", scheduleRefresh);
if (document.fonts?.ready) document.fonts.ready.then(scheduleRefresh);

// Merge-tool controls: write the result to Git's MERGED file and exit 0, or
// abort with a non-zero status that tells Git the merge was not resolved.
$("save-exit").addEventListener("click", async () => {
  try {
    await ipc.saveMerged(model.session_id);
    await ipc.quit(0);
  } catch (e) {
    $("status").textContent = `Save failed: ${e}`;
  }
});
$("abort").addEventListener("click", () => ipc.quit(1));

function setMergeToolMode(on: boolean) {
  $("merge-actions").style.display = on ? "flex" : "none";
}

async function boot() {
  if (inTauri) {
    const b = await ipc.bootstrap();
    if (b.mode === "merge" && b.model) {
      setMergeToolMode(true);
      apply(b.model);
    } else {
      setMergeToolMode(false);
      const demo = demoModel();
      apply(await ipc.openSession({ local: demo.localText, ancestor: demo.ancestorText, incoming: demo.incomingText }));
    }
  } else {
    setMergeToolMode(false);
    apply(demoModel().model);
  }
}
boot();
