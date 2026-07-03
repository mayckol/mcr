import type { SessionSummary, SessionProgress, Side } from "../ipc/types";

export interface FileListCallbacks {
  onSelect: (sessionId: string) => void;
  onAccept: (sessionId: string, from: Side) => void;
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!
  );
}

// The conflicted-file navigator. Lists every file the merge session manages, shows
// per-file status, and lets the user select a file or accept a whole side from the
// row. View-switching reuses the single MergeEditor via the onSelect callback.
export class FileList {
  constructor(
    private root: HTMLElement,
    private cb: FileListCallbacks
  ) {}

  render(files: SessionSummary[], activeId: string | null, progress: SessionProgress) {
    // Compare rows (change_status set) are read-only: a name-status badge instead
    // of the resolve dot, and no whole-file accept actions.
    const compare = files.length > 0 && files.every((f) => f.change_status);
    const head = compare
      ? `<div class="file-list-head">${progress.total} file(s) changed</div>`
      : `<div class="file-list-head">${progress.resolved_count} of ${progress.total} resolved</div>`;
    const rows = files
      .map((f) => {
        if (f.change_status) {
          const letter = escapeHtml(f.change_status);
          const rowCls = "file-row" + (f.session_id === activeId ? " active" : "");
          return (
            `<div class="${rowCls}" data-id="${f.session_id}">` +
            `<span class="file-change file-change-${letter}" title="${letter}">${letter}</span>` +
            `<span class="file-path" title="${escapeHtml(f.path_label)}">${escapeHtml(f.path_label)}</span>` +
            `</div>`
          );
        }
        const state = f.resolved ? "resolved" : f.kind === "text" ? "unresolved" : f.kind;
        const rowCls =
          "file-row" +
          (f.session_id === activeId ? " active" : "") +
          (f.resolved ? " is-resolved" : "");
        const accept = f.resolved
          ? ""
          : `<span class="file-accept">` +
            `<button class="accept-local" data-id="${f.session_id}" title="Accept ours (local) for this file">Ours</button>` +
            `<button class="accept-incoming" data-id="${f.session_id}" title="Accept theirs (incoming) for this file">Theirs</button></span>`;
        return (
          `<div class="${rowCls}" data-id="${f.session_id}">` +
          `<span class="file-status file-status-${f.resolved ? "resolved" : f.kind}" title="${state}"></span>` +
          `<span class="file-path" title="${escapeHtml(f.path_label)}">${escapeHtml(f.path_label)}</span>` +
          accept +
          `</div>`
        );
      })
      .join("");
    this.root.innerHTML = head + `<div class="file-rows">${rows}</div>`;

    this.root.querySelectorAll<HTMLElement>(".file-row").forEach((row) => {
      row.addEventListener("click", (e) => {
        const target = e.target as HTMLElement;
        const id = row.dataset.id!;
        if (target.classList.contains("accept-local")) {
          e.stopPropagation();
          this.cb.onAccept(id, "local");
          return;
        }
        if (target.classList.contains("accept-incoming")) {
          e.stopPropagation();
          this.cb.onAccept(id, "incoming");
          return;
        }
        this.cb.onSelect(id);
      });
    });
  }
}
