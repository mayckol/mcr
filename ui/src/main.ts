import { MergeEditor } from "./panes/merge";
import { ConnectorOverlay } from "./connectors/overlay";
import { ControlsLayer } from "./controls/layer";
import { syncScroll } from "./scroll/sync";
import { ipc, framedEmbed } from "./ipc/client";
import { EditorView } from "@codemirror/view";
import { demoModel } from "./demo";
import { Shortcuts } from "./shortcuts/keymap";
import { ShortcutsPanel } from "./shortcuts/panel";
import { FileList } from "./files/list";
import { ExitConfirmModal } from "./confirm/modal";
import { SettingsPanel } from "./settings/panel";
import { applyTheme, storedThemeId } from "./theme/manager";
import { applyFont } from "./theme/font";
import { emit, listen } from "@tauri-apps/api/event";
import type { SessionModel, SessionSummary, SessionProgress, Side } from "./ipc/types";

// Paint the persisted theme before any editor mounts: chrome, bands, gutters,
// and connectors all read the CSS vars this sets — one source of truth.
applyTheme(storedThemeId());

// Direct Tauri IPC exists only in a real webview main frame. In the Linux host's
// iframe embed there are no internals — commands ride the postMessage bridge in
// ipc/client.ts — so `inTauri` means "a backend answers our commands", by either
// transport, while `hasInternals` gates the Tauri event API specifically.
const hasInternals = "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
const inTauri = hasInternals || framedEmbed;

// Embedded inside a host app (fftracking) over its diff pane, one of two ways:
// a native child webview (macOS — the host injects `__FF_EMBED__` as an init
// script and drives it via the `mcr://embed-open` Tauri event), or a same-origin
// iframe (Linux, where tauri cannot position child webviews — the host loads
// /mcr/index.html?embed=1 and drives it via postMessage). Either way there is no
// CLI `Launch`; the file to show arrives at runtime.
const embed = (hasInternals && "__FF_EMBED__" in window) || framedEmbed;

const $ = (id: string) => document.getElementById(id)!;

let model: SessionModel | undefined;
let currentHunk: number | null = null;

// A scroll-to-change that is still waiting for the result pane to have a real
// height. In the embedded webview the host's git tab can be hidden (0-height,
// rAF paused) when a file first opens; the anchor is stashed here and completed
// the moment the pane is measurable (poll, or the visibility handler on show).
let anchorTarget: { line: number } | null = null;

// How the app was launched: git mergetool, `mcr diff <refA> <refB>`, or demo.
let appMode: "merge" | "compare" | "demo" = "demo";
// Compare mode: sessions changed since their last save to the working tree.
const dirty = new Set<string>();

// Multi-file session state. `files` is empty for demo / single-file fallback.
let files: SessionSummary[] = [];
let progress: SessionProgress = { total: 0, resolved_count: 0, remaining_conflicts: 0, all_resolved: false };
let activeFile: string | null = null;

const basename = (p?: string | null) => (p ? p.split("/").pop() || p : p ?? undefined);

// All backend session mutations run through one promise chain, so a debounced
// edit can never interleave with (and clobber) a hunk apply that raced past it.
let mutationChain: Promise<unknown> = Promise.resolve();
function enqueue<T>(fn: () => Promise<T>): Promise<T> {
  const next = mutationChain.then(fn, fn);
  mutationChain = next.catch(() => {});
  return next;
}

// Manual result edits are debounced: the old per-keystroke round-trip shipped the
// FULL document to the backend (which re-diffs it) on every character — typing in
// a large file crawled. Anything that reads or persists backend state must call
// flushEdit() first so no typed text is ever left behind.
const EDIT_DEBOUNCE_MS = 200;
let editTimer: number | null = null;
let pendingEdit: { sessionId: string; text: string } | null = null;

function queueEdit(sessionId: string, text: string) {
  pendingEdit = { sessionId, text };
  if (appMode === "compare") dirty.add(sessionId);
  if (editTimer !== null) clearTimeout(editTimer);
  editTimer = window.setTimeout(() => void flushEdit(), EDIT_DEBOUNCE_MS);
}

async function flushEdit(): Promise<void> {
  if (editTimer !== null) {
    clearTimeout(editTimer);
    editTimer = null;
  }
  const edit = pendingEdit;
  pendingEdit = null;
  if (!edit || !inTauri) return;
  try {
    await enqueue(() => ipc.editFullResult(edit.sessionId, edit.text));
  } catch (e) {
    $("status").textContent = `Edit failed: ${e}`;
  }
  scheduleRefresh();
}

const merge = new MergeEditor(
  { local: $("pane-local"), result: $("pane-result"), incoming: $("pane-incoming") },
  {
    onResultEdit: (fullText) => {
      // Free-form manual edit: persist the typed text backend-side WITHOUT
      // re-setting the result doc (which would reset the cursor mid-typing).
      if (!inTauri || !model) return;
      queueEdit(model.session_id, fullText);
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
    // Mid-layout or hidden (the embed's git tab is on another view): projecting
    // now would cull every band and repaint the overlay blank for a frame — the
    // "blink". Skip; the ResizeObserver/visibility handler re-fires this once
    // the pane has a real height.
    if (merge.result.scrollDOM.clientHeight === 0) return;
    // The pane just became measurable: land any scroll anchor that was parked
    // while it was hidden, in this same frame, so the reveal paints already
    // centered on the change instead of visibly jumping there.
    if (anchorTarget) tryAnchor(0);
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

// On opening a file, jump the result pane to its first (or, when arriving from
// backwards navigation, last) change and focus the editor. Called only on fresh
// loads — never after a mutation, which would yank the view.
function focusEdgeChange(edge: "first" | "last") {
  if (!model || model.hunks.length === 0) return;
  // Prefer the edge among unresolved changes so navigation lands on something
  // still actionable; fall back to all changes when the file is fully resolved.
  const unresolved = model.hunks.filter((h) => h.state.kind === "unresolved");
  const pool = unresolved.length > 0 ? unresolved : model.hunks;
  const target = pool.reduce((a, b) =>
    (edge === "first"
      ? b.result_range.start < a.result_range.start
      : b.result_range.start > a.result_range.start)
      ? b
      : a
  );
  currentHunk = target.id;
  anchorResultTo(target.result_range.start);
}

function focusFirstChange() {
  focusEdgeChange("first");
}

// Center the result pane on the given line, but only once the pane is actually
// measurable. Scrolling before layout settles (or while the embed tab is hidden,
// where the pane is 0-height) resolves against an unmeasured viewport and snaps
// the change to the document bottom — the "diff opens at the bottom" bug. We
// center rather than `scrollIntoView: true` (nearest) so the first change lands
// mid-pane with context, never hugging the bottom edge.
function anchorResultTo(line: number) {
  anchorTarget = { line };
  tryAnchor(0);
}

function tryAnchor(tries: number) {
  if (!model || !anchorTarget) return;
  const view = merge.result;
  if (view.scrollDOM.clientHeight > 0) {
    const startLine = anchorTarget.line;
    anchorTarget = null;
    const commit = () => {
      if (!model) return;
      const doc = view.state.doc;
      const pos = doc.line(Math.min(startLine + 1, doc.lines)).from;
      view.focus();
      view.dispatch({ selection: { anchor: pos }, effects: EditorView.scrollIntoView(pos, { y: "center" }) });
    };
    view.requestMeasure();
    commit();
    // A second pass after the measure settles pins the final position — WebKitGTK
    // finishes font metrics a frame late, which would otherwise leave it short.
    requestAnimationFrame(commit);
    return;
  }
  // Pane still 0-height. rAF is paused while the webview is hidden, so this
  // naturally resumes when the host shows the git tab; the cap only bounds a
  // slow first layout so we never poll forever.
  if (tries < 40) requestAnimationFrame(() => tryAnchor(tries + 1));
}

async function mutate(fn: (sessionId: string) => Promise<SessionModel>): Promise<SessionModel> {
  if (!inTauri || !model) return model as SessionModel; // demo: backend is a no-op
  return fn(model.session_id);
}

function act(fn: () => Promise<SessionModel>) {
  if (!model) return;
  flushEdit()
    .then(() => enqueue(fn))
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
      await enqueue(() => ipc.saveAndStage(next.session_id));
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
const settingsPanel = new SettingsPanel(
  (id) => applyTheme(id, (p) => merge.setTheme(p)),
  (settings) => merge.setFont(applyFont(settings))
);
$("settings").addEventListener("click", () => settingsPanel.open());
window.addEventListener("keydown", (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === ",") {
    e.preventDefault();
    settingsPanel.open();
  }
});
if (hasInternals) void listen("mcr://open-settings", () => settingsPanel.open());

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
async function selectFile(id: string, edge: "first" | "last" = "first") {
  await flushEdit(); // a pending edit belongs to the session being left
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
      focusEdgeChange(edge);
    } catch (e) {
      // Compare sessions build lazily; a file can turn out binary only here.
      $("status").textContent = `${summary?.path_label ?? "file"}: ${e}`;
      await refreshList();
      return;
    }
  }
  fileList.render(files, activeFile, progress);
}

async function onAccept(id: string, side: Side) {
  if (!inTauri) return;
  try {
    await flushEdit();
    await enqueue(() => ipc.acceptFile(id, side));
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
  await flushEdit();
  const id = await ipc.nextUnresolved(activeFile);
  if (id) await selectFile(id);
}
$("next-file").addEventListener("click", gotoNextUnresolved);

// Finish/exit the whole session. Single-file fallback keeps the legacy contract;
// the multi-file path stages resolved files and confirms before leaving any
// conflicts behind, exiting with the code Git expects for the file it passed.
async function exitFlow(abort: boolean) {
  if (!abort) await flushEdit();
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

// Files reachable by change navigation (binary entries never open in the editor).
function navigableFiles(): SessionSummary[] {
  return files.filter((f) => f.kind !== "binary");
}

// Next/previous change. At a file's last change, "next" continues at the first
// change of the next file in the list (wrapping); "prev" mirrors that backwards.
async function navigate(direction: "next" | "prev") {
  if (!inTauri || !model) return;
  await flushEdit();

  // The backend returns the next *unresolved* change in this direction (resolved
  // changes — the dotted ghosts — are skipped), or null when none remain.
  const id = await ipc.navigate(model.session_id, direction, currentHunk);
  if (id !== null) {
    currentHunk = id;
    const h = model.hunks.find((x) => x.id === id);
    if (!h) return;
    const line = merge.result.state.doc.line(Math.min(h.result_range.start + 1, merge.result.state.doc.lines));
    merge.result.dispatch({ selection: { anchor: line.from }, scrollIntoView: true });
    return;
  }

  // No unresolved change left this direction in the current file: continue at the
  // adjacent navigable file (wrapping), landing on its first/last unresolved change.
  const nav = navigableFiles();
  if (nav.length > 1) {
    const found = nav.findIndex((f) => f.session_id === activeFile);
    // Unknown active file: "next" starts at the first file, "prev" at the last.
    const cur = found === -1 ? (direction === "next" ? nav.length - 1 : 0) : found;
    const next =
      direction === "next"
        ? nav[(cur + 1) % nav.length]
        : nav[(cur - 1 + nav.length) % nav.length];
    await selectFile(next.session_id, direction === "next" ? "first" : "last");
  }
}

syncScroll(merge.views(), merge.result, scheduleRefresh);
new ResizeObserver(scheduleRefresh).observe(container);
window.addEventListener("resize", scheduleRefresh);
if (document.fonts?.ready) document.fonts.ready.then(scheduleRefresh);

// Embedded, the webview is hidden while the host shows another tab; WebKitGTK
// pauses rendering and any geometry measured meanwhile is stale. On becoming
// visible again, re-measure all panes and re-project the overlay so connectors
// don't blink in — and complete a scroll anchor that was waiting on real height.
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState !== "visible" || !model) return;
  for (const v of merge.views()) v.requestMeasure();
  if (anchorTarget) tryAnchor(0);
  scheduleRefresh();
});

// Compare mode: Save writes every changed session to its working-tree file (the
// window stays open); Close exits 0, confirming first when edits are unsaved.
async function compareSave() {
  await flushEdit();
  const ids = [...dirty];
  try {
    for (const id of ids) {
      await enqueue(() => ipc.saveMerged(id));
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

// Compare is two panes: the ref on the left, the editable working file on the
// right. The incoming pane is hidden — it would only duplicate the current file.
function setCompareMode(ref: string) {
  $("merge-actions").style.display = "none";
  $("footbar").style.display = "flex";
  $("foot-accept-left").style.display = "none";
  $("foot-accept-right").style.display = "none";
  const save = $("foot-apply");
  save.textContent = "Save";
  save.title = "Write the current version to the working tree";
  const close = $("foot-cancel");
  close.textContent = "Close";
  close.title = "Close the compare window";
  $("title-local").textContent = ref;
  $("title-result").textContent = "Current version";
  container.classList.add("two-pane");
  document.title = `MCR — ${ref} ↔ working tree`;
}

// The file currently shown in embed mode. The host re-fires `embed-open` for the
// same file whenever its git tab is reactivated (not only on a new selection), so
// we detect that and refresh in place instead of reopening from scratch.
let embedTarget: string | null = null;

// Host asked to show a specific file: open its compare session and render it.
async function openEmbedded(p: { repoRoot: string; refspec: string; path: string }) {
  const key = `${p.repoRoot} ${p.refspec} ${p.path}`;
  const sameFile = key === embedTarget && !!model;
  try {
    const m = await ipc.compareOpen(p.repoRoot, p.refspec, p.path);
    embedTarget = key;
    merge.setLanguage(basename(p.path));
    setCompareMode(p.refspec);
    if (sameFile) {
      // Tab reactivation: the working tree may have changed, so refresh the diff,
      // but keep the user where they were — re-anchoring to the first change and
      // stealing focus on every switch is what read as blinking/janky.
      const top = merge.result.scrollDOM.scrollTop;
      apply(m);
      const restore = () => {
        merge.result.scrollDOM.scrollTop = top;
        merge.local.scrollDOM.scrollTop = top;
      };
      restore();
      requestAnimationFrame(restore);
    } else {
      apply(m);
      focusFirstChange();
    }
    // On WebKitGTK the host reveals the webview by repositioning it (it is kept
    // mapped and merely parked off-screen, so `visibilitychange` never fires
    // here). `embed-open` is the signal that always arrives on a Git-tab show:
    // re-measure so a pane that settled while parked re-projects its overlay, and
    // finish any scroll anchor that was still waiting on a real height.
    for (const v of merge.views()) v.requestMeasure();
    if (anchorTarget) tryAnchor(0);
  } catch (e) {
    $("status").textContent = `${p.path}: ${e}`;
  }
}

// Embedded boot: no repository scan and no file list — the two-pane compare
// chrome is fixed, and the host drives which file is shown. Transport depends on
// how we're hosted: Tauri events in a native child webview, postMessage in the
// Linux iframe. Either way, announce readiness once the handler is registered so
// a file selected before this frame finished booting still (re)arrives.
async function bootEmbedded() {
  appMode = "compare";
  document.body.classList.add("ff-embed");
  showFileList(false);
  if (framedEmbed) {
    window.addEventListener("message", (e) => {
      if (e.source !== window.parent) return;
      const d = e.data as { mcr?: string; repoRoot?: string; refspec?: string; path?: string };
      if (d && d.mcr === "open" && d.repoRoot && d.refspec && d.path) {
        void openEmbedded({ repoRoot: d.repoRoot, refspec: d.refspec, path: d.path });
      }
    });
    window.parent.postMessage({ mcr: "ready" }, "*");
  } else {
    await listen<{ repoRoot: string; refspec: string; path: string }>(
      "mcr://embed-open",
      (e) => void openEmbedded(e.payload),
    );
    await emit("mcr://embed-ready", {});
  }
  $("status").textContent = "";
}

async function boot() {
  if (embed) return bootEmbedded();
  if (inTauri) {
    // Discovery now runs inside bootstrap (the window opens before any repo
    // scan) — tell the user what the wait is while a big repository is listed.
    $("status").textContent = "Scanning repository…";
    let b;
    try {
      b = await ipc.bootstrap();
    } catch (e) {
      $("status").textContent = `Failed to read the repository: ${e}`;
      return;
    }
    $("status").textContent = "";
    if (b.mode === "merge" || b.mode === "compare") {
      appMode = b.mode;
      if (b.mode === "compare") {
        setCompareMode(b.compare_ref ?? "ref");
      } else {
        setMergeToolMode(true);
      }
      files = b.files;
      progress = b.progress;
      // The list is the entry point only when more than one file conflicts
      // (FR-001); a single conflicted file opens straight into the editor
      // (FR-015) — unless it could not open as text (binary), in which case the
      // list is the only place its accept buttons live.
      if (files.length > 1 || (files.length === 1 && !b.active)) {
        showFileList(true);
        fileList.render(files, activeFile, progress);
      }
      if (b.active) {
        activeFile = b.active.session_id;
        merge.setLanguage(b.file_name);
        apply(b.active);
        focusFirstChange();
      } else if (b.mode === "compare" && files.length === 0) {
        $("status").textContent = `No differences between ${b.compare_ref} and the working tree`;
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
