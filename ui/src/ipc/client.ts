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

export interface OpenInput {
  local: string;
  ancestor: string;
  incoming: string;
  whitespace_mode?: "none" | "ignore_trailing" | "ignore_all";
}

export interface Bootstrap {
  mode: "merge" | "demo";
  files: SessionSummary[];
  progress: SessionProgress;
  active: SessionModel | null;
  file_name?: string | null;
}

export const ipc = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),

  listSessions: () => invoke<[SessionSummary[], SessionProgress]>("list_sessions"),

  selectSession: (sessionId: string) =>
    invoke<SessionModel>("select_session", { sessionId }),

  saveAndStage: (sessionId: string) => invoke<void>("save_and_stage", { sessionId }),

  acceptFile: (sessionId: string, from: Side) =>
    invoke<SessionSummary>("accept_file", { sessionId, from }),

  nextUnresolved: (current: string | null) =>
    invoke<string | null>("next_unresolved", { current }),

  finish: () => invoke<FinishOutcome>("finish"),

  exitCode: () => invoke<number>("exit_code"),

  saveMerged: (sessionId: string) => invoke<void>("save_merged", { sessionId }),

  quit: (code: number) => invoke<void>("quit", { code }),

  openSession: (input: OpenInput) =>
    invoke<SessionModel>("open_session", input as unknown as Record<string, unknown>),

  applyChange: (sessionId: string, hunkId: number, from: Side) =>
    invoke<SessionModel>("apply_change", { sessionId, hunkId, from }),

  applyBoth: (sessionId: string, hunkId: number, first: Side) =>
    invoke<SessionModel>("apply_both", { sessionId, hunkId, first }),

  revertChange: (sessionId: string, hunkId: number) =>
    invoke<SessionModel>("revert_change", { sessionId, hunkId }),

  applyNonConflicting: (sessionId: string, from: Side | "both") =>
    invoke<SessionModel>("apply_non_conflicting", { sessionId, from }),

  editResult: (sessionId: string, start: number, end: number, text: string) =>
    invoke<SessionModel>("edit_result", { sessionId, start, end, text }),

  editFullResult: (sessionId: string, text: string) =>
    invoke<SessionModel>("edit_full_result", { sessionId, text }),

  undo: (sessionId: string) => invoke<SessionModel>("undo", { sessionId }),
  redo: (sessionId: string) => invoke<SessionModel>("redo", { sessionId }),

  navigate: (sessionId: string, direction: "next" | "prev", fromHunk: number | null) =>
    invoke<number | null>("navigate", { sessionId, direction, fromHunk }),

  setWhitespaceMode: (sessionId: string, mode: "none" | "ignore_trailing" | "ignore_all") =>
    invoke<SessionModel>("set_whitespace_mode", { sessionId, mode }),
};
