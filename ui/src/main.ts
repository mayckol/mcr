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
import { SettingsPanel } from "./settings/panel";
import { applyTheme, storedThemeId } from "./theme/manager";
import { listen } from "@tauri-apps/api/event";
import type { SessionModel, SessionSummary, SessionProgress, Side } from "./ipc/types";

// Paint the persisted theme before any editor mounts: chrome, bands, gutters,
// and connectors all read the CSS vars this sets — one source of truth.
applyTheme(storedThemeId());

const inTauri = "__TAURI_INTERNALS__" in window || "__TAURI__" in window;

const $ = (id: string) => document.getElementById(id)!;

let model: SessionModel | undefined;
let currentHunk: number | null = null;

// How the app was launched: git mergetool, `mcr diff <refA> <refB>`, or demo.
let appMode: "merge" | "compare" | "demo" = "demo";
// Compare mode: sessions changed since their last save to the working tree.
const dirty = new Set<string>();

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
        if (appMode === "compare") dirty.add(model.session_id);
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

// On first opening a file, jump the result pane to its earliest change and focus
// the editor, so review starts at the first hunk instead of the top of the file.
// Called only on fresh loads — never after a mutation, which would yank the view.
function focusFirstChange() {
  if (!model || model.hunks.length === 0) return;
  const first = model.hunks.reduce((a, b) =>
    b.result_range.start < a.result_range.start ? b : a
  );
  currentHunk = first.id;
  // Defer past the webview's font/layout measurement (same reason apply() re-runs
  // scheduleRefresh on a delay); scrolling before layout settles snaps to the top.
  const go = () => {
    if (!model) return;
    const doc = merge.result.state.doc;
    const line = doc.line(Math.min(first.result_range.start + 1, doc.lines));
    merge.result.focus();
    merge.result.dispatch({ selection: { anchor: line.from }, scrollIntoView: true });
  };
  requestAnimationFrame(go);
  setTimeout(go, 120);
}

async function mutate(fn: (sessionId: string) => Promise<SessionModel>): Promise<SessionModel> {
  if (!inTauri || !model) return model as SessionModel; // demo: backend is a no-op
  return fn(model.session_id);
}

function act(fn: () => Promise<SessionModel>) {
  if (!model) return;
  fn()
    .then(async (next) => {
      if (appMode === "compare") dirty.add(next.session_id);
      apply(next);
      await afterMutate(next);
    })
    .catch((e) => {
      $("status").textContent = `Action failed: ${e}`;
    });
}

// After a mutation: in a multi-file session, persist a file the moment it is fully
// resolved (incremental persist, FR-017) and refresh the list/progress (FR-005/006).
async function afterMutate(next: SessionModel) {
  if (!inTauri || files.length === 0) return;
  // Compare mode persists only on explicit Save — never auto-stage.
  if (appMode !== "compare" && next.status.fully_resolved) {
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
    if (files.length > 0) {
      $("status").textContent =
        appMode === "compare"
          ? `${progress.total} file(s) changed`
          : `${progress.remaining_conflicts} file(s) with conflicts`;
    }
    return;
  }
  const s = model.status;
  if (appMode === "compare") {
    $("status").textContent = `${s.total_hunks} difference(s)`;
    return;
  }
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

// Settings (Appearance). Opened from the gear, Cmd/Ctrl+,, or the native
// macOS app-menu "Settings…" item (which emits mcr://open-settings).
const settingsPanel = new SettingsPanel((id) => applyTheme(id, (p) => merge.setTheme(p)));
$("settings").addEventListener("click", () => settingsPanel.open());
window.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === ",") {
    e.preventDefault();
    settingsPanel.open();
  }
});
if (inTauri) void listen("mcr://open-settings", () => settingsPanel.open());

// Multi-file navigator + exit-confirmation modal.
const fileList = new FileList($("file-list"), { onSelect: selectFile, onAccept });
const exitModal = new ExitConfirmModal();

function showFileList(on: boolean) {
  $("file-list").style.display = on ? "flex" : "none";
  // "Next unresolved" is a merge-only affordance.
  $("next-file").style.display = on && appMode === "merge" ? "inline-block" : "none";
}

// Open a file from the list in the shared three-pane editor. Special (non-text)
// conflicts are resolved only via accept, so they do not open the editor (FR-014).
async function selectFile(id: string) {
  const summary = files.find((f) => f.session_id === id);
  activeFile = id;
  // Binary blobs cannot be shown as text — they stay accept-only. Text, both-added
  // and delete/modify all reconstruct as text sides, so they open in the editor:
  // a deleted side renders as an empty (black) pane against the other side's diff.
  if (summary && summary.kind === "binary") {
    $("status").textContent =
      appMode === "compare"
        ? `${summary.path_label}: binary file — cannot compare as text`
        : `${summary.path_label}: binary conflict — use Accept Ours / Theirs`;
    fileList.render(files, activeFile, progress);
    return;
  }
  if (inTauri) {
    try {
      const m = await ipc.selectSession(id);
      merge.setLanguage(basename(summary?.path_label));
      apply(m);
      focusFirstChange();
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
  if (abort) {
    // Abort must never save, stage, or exit 0 — a non-zero exit tells Git the
    // passed file stays conflicted. Files already staged this session are kept
    // by Git; everything in-progress is discarded, hence the confirmation.
    exitModal.confirm({
      title: "Abort merge?",
      body:
        "Exit without applying. The file stays conflicted for Git; unsaved " +
        "resolutions in this window are discarded.",
      okLabel: "Abort",
      onConfirm: () => void ipc.quit(1),
    });
    return;
  }
  if (files.length === 0) {
    if (model) {
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

// Compare mode: Save writes every changed session to its working-tree file (the
// window stays open); Close exits 0, confirming first when edits are unsaved.
async function compareSave() {
  const ids = [...dirty];
  try {
    for (const id of ids) {
      await ipc.saveMerged(id);
      dirty.delete(id);
    }
    $("status").textContent = `Saved ${ids.length} file(s) to working tree`;
  } catch (e) {
    $("status").textContent = `Save failed: ${e}`;
  }
}

function compareClose() {
  if (dirty.size > 0) {
    exitModal.confirm({
      title: "Close with unsaved changes?",
      body: `${dirty.size} file(s) have edits that were not saved to the working tree. Closing discards them.`,
      okLabel: "Close",
      cancelLabel: "Keep editing",
      onConfirm: () => void ipc.quit(0),
    });
    return;
  }
  void ipc.quit(0);
}

// Footer action bar. Accept Left/Right apply all non-conflicting changes from a
// side; Apply writes the result to Git's MERGED file and exits 0; Cancel aborts
// with a non-zero status that tells Git the merge was not resolved. In compare
// mode the same buttons become Save / Close.
$("foot-accept-left").addEventListener("click", actions.applyLeft);
$("foot-accept-right").addEventListener("click", actions.applyRight);
$("foot-apply").addEventListener("click", () => {
  if (appMode === "compare") void compareSave();
  else void exitFlow(false);
});
$("foot-cancel").addEventListener("click", () => {
  if (appMode === "compare") compareClose();
  else void exitFlow(true);
});

function setMergeToolMode(on: boolean) {
  $("merge-actions").style.display = on ? "flex" : "none";
  $("footbar").style.display = on ? "flex" : "none";
}

function setCompareMode(refA: string, refB: string) {
  $("merge-actions").style.display = "none";
  $("footbar").style.display = "flex";
  $("foot-accept-left").style.display = "none";
  $("foot-accept-right").style.display = "none";
  const save = $("foot-apply");
  save.textContent = "Save";
  save.title = "Write the result to the working tree";
  const close = $("foot-cancel");
  close.textContent = "Close";
  close.title = "Close the compare window";
  $("title-local").textContent = refA;
  $("title-result").textContent = "Working tree";
  $("title-incoming").textContent = refB;
  document.title = `MCR — ${refA} ↔ ${refB}`;
}

async function boot() {
  if (inTauri) {
    const b = await ipc.bootstrap();
    if (b.mode === "merge" || b.mode === "compare") {
      appMode = b.mode;
      if (b.mode === "compare") {
        setCompareMode(b.ref_a ?? "A", b.ref_b ?? "B");
      } else {
        setMergeToolMode(true);
      }
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
        focusFirstChange();
      } else if (b.mode === "compare" && files.length === 0) {
        $("status").textContent = `No differences between ${b.ref_a} and ${b.ref_b}`;
      } else {
        // More than one file: show the list first, no editor yet (FR-001).
        renderStatus();
      }
    } else {
      setMergeToolMode(false);
      const demo = demoModel();
      apply(await ipc.openSession({ local: demo.localText, ancestor: demo.ancestorText, incoming: demo.incomingText }));
      focusFirstChange();
    }
  } else {
    setMergeToolMode(false);
    apply(demoModel().model);
    focusFirstChange();
  }
}
boot();
