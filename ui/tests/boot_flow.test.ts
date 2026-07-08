// Boot-flow smoke test against a mocked Tauri bridge: bootstrap loading state,
// lazy file selection, debounced result edits, and the edit-before-mutation
// ordering guarantee (a debounced edit must never land after a hunk apply).
import { describe, it, expect, beforeAll } from "vitest";
import type { SessionModel, SessionSummary, SessionProgress } from "../src/ipc/types";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function model(id: string, result = "one\ntwo\nthree"): SessionModel {
  const lines = result.split("\n");
  return {
    session_id: id,
    panes: { local: lines, result: lines, incoming: lines },
    alignment: lines.map((_, i) => ({ local: i, result: i, incoming: i, hunk: null })),
    hunks: [],
    status: { total_hunks: 0, remaining_conflicts: 0, fully_resolved: false },
  };
}

const summaries: SessionSummary[] = ["a.txt", "b.txt", "sub/c.txt"].map((p, i) => ({
  session_id: `session-${i + 1}`,
  path_label: p,
  kind: "text",
  resolved: false,
  remaining_conflicts: 0,
}));
const progress: SessionProgress = {
  total: 3,
  resolved_count: 0,
  remaining_conflicts: 3,
  all_resolved: false,
};

const calls: { cmd: string; args: Record<string, unknown> }[] = [];
let releaseBootstrap: (() => void) | undefined;

function mockTauri() {
  (window as any).__TAURI_INTERNALS__ = {
    transformCallback: () => 1,
    invoke: (cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd.startsWith("plugin:")) return Promise.resolve(1);
      calls.push({ cmd, args });
      switch (cmd) {
        case "bootstrap":
          return new Promise((resolve) => {
            releaseBootstrap = () =>
              resolve({
                mode: "merge",
                files: summaries,
                progress,
                active: null,
                file_name: null,
              });
          });
        case "select_session":
          return Promise.resolve(model(args.sessionId as string));
        case "edit_full_result":
          return Promise.resolve(model(args.sessionId as string, args.text as string));
        case "apply_non_conflicting":
        case "apply_change":
          return Promise.resolve(model(args.sessionId as string));
        case "list_sessions":
          return Promise.resolve([summaries, progress]);
        case "navigate":
          return Promise.resolve(null);
        default:
          return Promise.resolve(null);
      }
    },
  };
}

beforeAll(async () => {
  if (!("ResizeObserver" in globalThis)) {
    (globalThis as any).ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
  document.body.innerHTML = `
    <header class="toolbar">
      <button id="apply-left"></button><button id="apply-all"></button><button id="apply-right"></button>
      <button id="prev"></button><button id="next"></button>
      <button id="undo"></button><button id="redo"></button>
      <select id="whitespace"><option value="none"></option></select>
      <button id="shortcuts"></button><button id="settings"></button>
      <span id="merge-actions" style="display:none"><button id="next-file"></button></span>
      <span id="status"></span>
    </header>
    <div id="work">
      <aside id="file-list" style="display:none"></aside>
      <div id="merge-container">
        <section><div id="title-local"></div><div id="pane-local"></div></section>
        <section><div id="title-result"></div><div id="pane-result"></div></section>
        <section><div id="title-incoming"></div><div id="pane-incoming"></div></section>
      </div>
    </div>
    <footer id="footbar" style="display:none">
      <button id="foot-accept-left"></button><button id="foot-accept-right"></button>
      <button id="foot-cancel"></button><button id="foot-apply"></button>
    </footer>`;
  mockTauri();
  await import("../src/main");
});

describe("boot flow with deferred discovery", () => {
  it("shows the scanning state while bootstrap runs, then the file list", async () => {
    expect(document.getElementById("status")!.textContent).toBe("Scanning repository…");
    releaseBootstrap!();
    await sleep(20);
    const rows = document.querySelectorAll("#file-list .file-row");
    expect(rows.length).toBe(3);
    expect(document.querySelector("#file-list .file-list-head")!.textContent).toContain(
      "0 of 3 resolved"
    );
  });

  it("materializes a session lazily on selection", async () => {
    (document.querySelector('#file-list .file-row[data-id="session-1"]') as HTMLElement).click();
    await sleep(20);
    expect(calls.some((c) => c.cmd === "select_session" && c.args.sessionId === "session-1")).toBe(
      true
    );
    const { viewHolder } = await import("../src/highlight/decorations");
    expect(viewHolder.get("result")!.state.doc.toString()).toBe("one\ntwo\nthree");
  });

  it("debounces manual edits into one backend round-trip", async () => {
    const { viewHolder } = await import("../src/highlight/decorations");
    const view = viewHolder.get("result")!;
    view.dispatch({ changes: { from: 0, insert: "x" } });
    view.dispatch({ changes: { from: 0, insert: "y" } });
    view.dispatch({ changes: { from: 0, insert: "z" } });
    expect(calls.filter((c) => c.cmd === "edit_full_result").length).toBe(0);
    await sleep(350);
    const edits = calls.filter((c) => c.cmd === "edit_full_result");
    expect(edits.length).toBe(1);
    expect(edits[0].args.text).toBe("zyxone\ntwo\nthree");
  });

  it("flushes a pending edit before a mutation so ordering holds", async () => {
    const before = calls.length;
    const { viewHolder } = await import("../src/highlight/decorations");
    viewHolder.get("result")!.dispatch({ changes: { from: 0, insert: "Q" } });
    // Immediately mutate — well inside the debounce window.
    (document.getElementById("apply-all") as HTMLElement).click();
    await sleep(350);
    const seq = calls.slice(before).map((c) => c.cmd);
    const editAt = seq.indexOf("edit_full_result");
    const applyAt = seq.indexOf("apply_non_conflicting");
    expect(editAt).toBeGreaterThanOrEqual(0);
    expect(applyAt).toBeGreaterThanOrEqual(0);
    expect(editAt).toBeLessThan(applyAt);
    expect(seq.filter((c) => c === "edit_full_result").length).toBe(1);
  });
});
