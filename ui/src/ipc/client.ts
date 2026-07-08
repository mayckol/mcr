import { invoke } from "@tauri-apps/api/core";
import type {
  SessionModel,
  Side,
  SessionSummary,
  SessionProgress,
  FinishOutcome,
} from "./types";

// Typed wrappers over the Tauri command surface (contracts/ipc-merge-session.md).
// The UI only dispatches intents through these; it never derives merge state.

// Embedded as a same-origin iframe (the Linux host): Tauri injects its IPC
// bootstrap into the main frame only, so a subframe has no `__TAURI_INTERNALS__`
// and cannot invoke directly. Every command instead rides a postMessage bridge —
// `{mcr:"invoke", id, cmd, args}` up to the host page, which executes the real
// invoke and answers `{mcr:"result", id, ok, value|error}`.
export const framedEmbed = (() => {
  try {
    return window.parent !== window && new URLSearchParams(window.location.search).has("embed");
  } catch {
    return false;
  }
})();
const hasInternals = "__TAURI_INTERNALS__" in window || "__TAURI__" in window;

interface PendingCall {
  resolve: (v: unknown) => void;
  reject: (e: unknown) => void;
}
const pending = new Map<number, PendingCall>();
let callId = 0;

if (framedEmbed && !hasInternals) {
  window.addEventListener("message", (e) => {
    if (e.source !== window.parent) return;
    const d = e.data as { mcr?: string; id?: number; ok?: boolean; value?: unknown; error?: unknown };
    if (!d || d.mcr !== "result" || typeof d.id !== "number") return;
    const p = pending.get(d.id);
    if (!p) return;
    pending.delete(d.id);
    if (d.ok) p.resolve(d.value);
    else p.reject(d.error);
  });
}

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!framedEmbed || hasInternals) return invoke<T>(cmd, args);
  return new Promise<T>((resolve, reject) => {
    const id = ++callId;
    pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
    window.parent.postMessage({ mcr: "invoke", id, cmd, args }, "*");
  });
}

export interface OpenInput {
  local: string;
  ancestor: string;
  incoming: string;
  whitespace_mode?: "none" | "ignore_trailing" | "ignore_all";
}

export interface Bootstrap {
  mode: "merge" | "demo" | "compare";
  files: SessionSummary[];
  progress: SessionProgress;
  active: SessionModel | null;
  file_name?: string | null;
  /** Compare mode only: the ref the working tree is compared against. */
  compare_ref?: string | null;
}

export const ipc = {
  bootstrap: () => call<Bootstrap>("bootstrap"),

  listSessions: () => call<[SessionSummary[], SessionProgress]>("list_sessions"),

  selectSession: (sessionId: string) =>
    call<SessionModel>("select_session", { sessionId }),

  compareOpen: (root: string, refspec: string, path: string) =>
    call<SessionModel>("compare_open", { root, refspec, path }),

  saveAndStage: (sessionId: string) => call<void>("save_and_stage", { sessionId }),

  acceptFile: (sessionId: string, from: Side) =>
    call<SessionSummary>("accept_file", { sessionId, from }),

  nextUnresolved: (current: string | null) =>
    call<string | null>("next_unresolved", { current }),

  finish: () => call<FinishOutcome>("finish"),

  exitCode: () => call<number>("exit_code"),

  saveMerged: (sessionId: string) => call<void>("save_merged", { sessionId }),

  quit: (code: number) => call<void>("quit", { code }),

  openSession: (input: OpenInput) =>
    call<SessionModel>("open_session", input as unknown as Record<string, unknown>),

  applyChange: (sessionId: string, hunkId: number, from: Side) =>
    call<SessionModel>("apply_change", { sessionId, hunkId, from }),

  applyBoth: (sessionId: string, hunkId: number, first: Side) =>
    call<SessionModel>("apply_both", { sessionId, hunkId, first }),

  revertChange: (sessionId: string, hunkId: number) =>
    call<SessionModel>("revert_change", { sessionId, hunkId }),

  applyNonConflicting: (sessionId: string, from: Side | "both") =>
    call<SessionModel>("apply_non_conflicting", { sessionId, from }),

  editResult: (sessionId: string, start: number, end: number, text: string) =>
    call<SessionModel>("edit_result", { sessionId, start, end, text }),

  editFullResult: (sessionId: string, text: string) =>
    call<SessionModel>("edit_full_result", { sessionId, text }),

  undo: (sessionId: string) => call<SessionModel>("undo", { sessionId }),
  redo: (sessionId: string) => call<SessionModel>("redo", { sessionId }),

  navigate: (sessionId: string, direction: "next" | "prev", fromHunk: number | null) =>
    call<number | null>("navigate", { sessionId, direction, fromHunk }),

  setWhitespaceMode: (sessionId: string, mode: "none" | "ignore_trailing" | "ignore_all") =>
    call<SessionModel>("set_whitespace_mode", { sessionId, mode }),
};
