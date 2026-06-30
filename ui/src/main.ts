import { MergeEditor } from "./panes/merge";
import { ConnectorOverlay } from "./connectors/overlay";
import { ControlsLayer } from "./controls/layer";
import { syncScroll } from "./scroll/sync";
import { ipc } from "./ipc/client";
import { demoModel } from "./demo";
import { Shortcuts } from "./shortcuts/keymap";
import { ShortcutsPanel } from "./shortcuts/panel";
import { FileList } from "./files/list";
import { ExitConfirmModal } from "./confirm/modal";
import type { SessionModel, SessionSummary, SessionProgress, Side } from "./ipc/types";

const inTauri = "__TAURI_INTERNALS__" in window || "__TAURI__" in window;

const $ = (id: string) => document.getElementById(id)!;

let model: SessionModel | undefined;
let currentHunk: number | null = null;

// Multi-file session state. `files` is empty for demo / single-file fallback.
let files: SessionSummary[] = [];
let progress: SessionProgress = { total: 0, resolved_count: 0, remaining_conflicts: 0, all_resolved: false };
let activeFile: string | null = null;

const basename = (p?: string | null) => (p ? p.split("/").pop() || p : p ?? undefined);

const merge = new MergeEditor(
  { local: $("pane-local"), result: $("pane-result"), incoming: $("pane-incoming") },
  {
    onResultEdit: async (fullText) => {
      // Free-form manual edit: persist the typed text backend-side WITHOUT
      // re-setting the result doc (which would reset the cursor mid-typing).
      if (!inTauri || !model) return;
      try {
        await ipc.editFullResult(model.session_id, fullText);
      } catch (e) {
        $("status").textContent = `Edit failed: ${e}`;
      }
      scheduleRefresh();
    },
    onGeometryChange: () => scheduleRefresh(),
  }
);

const container = $("merge-container");
const overlay = new ConnectorOverlay(container, merge.local, merge.result, merge.incoming);
const controls = new ControlsLayer(container, merge.local, merge.result, merge.incoming, {
  applyLocal: (id) => act(() => mutate((s) => ipc.applyChange(s, id, "local"))),
  applyIncoming: (id) => act(() => mutate((s) => ipc.applyChange(s, id, "incoming"))),
  acceptBoth: (id, first) => act(() => mutate((s) => ipc.applyBoth(s, id, first))),
  revert: (id) => act(() => mutate((s) => ipc.revertChange(s, id))),
});

let rafPending = false;
function scheduleRefresh() {
  if (rafPending) return;
  rafPending = true;
  requestAnimationFrame(() => {
    rafPending = false;
    if (!model) return;
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
  if (!inTauri || !model) return model as SessionModel; // demo: backend is a no-op
  return fn(model.session_id);
}

function act(fn: () => Promise<SessionModel>) {
  if (!model) return;
  fn().then(async (next) => {
    apply(next);
    await afterMutate(next);
  });
}

// After a mutation: in a multi-file session, persist a file the moment it is fully
// resolved (incremental persist, FR-017) and refresh the list/progress (FR-005/006).
async function afterMutate(next: SessionModel) {
  if (!inTauri || files.length === 0) return;
  if (next.status.fully_resolved) {
    try {
      await ipc.saveAndStage(next.session_id);
    } catch (e) {
      $("status").textContent = `Save failed: ${e}`;
    }
  }
  await refreshList();
}

async function refreshList() {
  if (!inTauri || files.length === 0) return;
  const [f, p] = await ipc.listSessions();
  files = f;
  progress = p;
  fileList.render(files, activeFile, progress);
}

function renderStatus() {
  if (!model) {
    if (files.length > 0) $("status").textContent = `${progress.remaining_conflicts} file(s) with conflicts`;
    return;
  }
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

// Multi-file navigator + exit-confirmation modal.
const fileList = new FileList($("file-list"), { onSelect: selectFile, onAccept });
const exitModal = new ExitConfirmModal();

function showFileList(on: boolean) {
  $("file-list").style.display = on ? "flex" : "none";
  $("next-file").style.display = on ? "inline-block" : "none";
}

// Open a file from the list in the shared three-pane editor. Special (non-text)
// conflicts are resolved only via accept, so they do not open the editor (FR-014).
async function selectFile(id: string) {
  const summary = files.find((f) => f.session_id === id);
  activeFile = id;
  if (summary && summary.kind !== "text") {
    $("status").textContent = `${summary.path_label}: ${summary.kind} conflict — use Accept Local / Incoming`;
    fileList.render(files, activeFile, progress);
    return;
  }
  if (inTauri) {
    try {
      const m = await ipc.selectSession(id);
      merge.setLanguage(basename(summary?.path_label));
      apply(m);
    } catch (e) {
      $("status").textContent = `Open failed: ${e}`;
    }
  }
  fileList.render(files, activeFile, progress);
}

async function onAccept(id: string, side: Side) {
  if (!inTauri) return;
  try {
    await ipc.acceptFile(id, side);
    await refreshList();
    if (id === activeFile) {
      const summary = files.find((f) => f.session_id === id);
      if (summary && summary.kind === "text") apply(await ipc.selectSession(id));
    }
  } catch (e) {
    $("status").textContent = `Accept failed: ${e}`;
  }
}

async function gotoNextUnresolved() {
  if (!inTauri || files.length === 0) return;
  const id = await ipc.nextUnresolved(activeFile);
  if (id) await selectFile(id);
}
$("next-file").addEventListener("click", gotoNextUnresolved);

// Finish/exit the whole session. Single-file fallback keeps the legacy contract;
// the multi-file path stages resolved files and confirms before leaving any
// conflicts behind, exiting with the code Git expects for the file it passed.
async function exitFlow(abort: boolean) {
  if (files.length === 0) {
    if (abort) {
      await ipc.quit(1);
    } else if (model) {
      try {
        await ipc.saveMerged(model.session_id);
        await ipc.quit(0);
      } catch (e) {
        $("status").textContent = `Save failed: ${e}`;
      }
    }
    return;
  }
  try {
    const outcome = await ipc.finish();
    if (outcome.all_resolved) {
      await ipc.quit(0);
      return;
    }
    exitModal.open(outcome.unresolved, async () => {
      await ipc.quit(await ipc.exitCode());
    });
  } catch (e) {
    $("status").textContent = `Finish failed: ${e}`;
  }
}

async function navigate(direction: "next" | "prev") {
  if (!inTauri || !model) return;
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
$("save-exit").addEventListener("click", () => exitFlow(false));
$("abort").addEventListener("click", () => exitFlow(true));

function setMergeToolMode(on: boolean) {
  $("merge-actions").style.display = on ? "flex" : "none";
}

async function boot() {
  if (inTauri) {
    const b = await ipc.bootstrap();
    if (b.mode === "merge") {
      setMergeToolMode(true);
      files = b.files;
      progress = b.progress;
      // The list is the entry point only when more than one file conflicts
      // (FR-001); a single conflicted file opens straight into the editor (FR-015).
      if (files.length > 1) {
        showFileList(true);
        fileList.render(files, activeFile, progress);
      }
      if (b.active) {
        activeFile = b.active.session_id;
        merge.setLanguage(b.file_name);
        apply(b.active);
      } else {
        // More than one file: show the list first, no editor yet (FR-001).
        renderStatus();
      }
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
