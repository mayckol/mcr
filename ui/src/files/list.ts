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

interface DirNode {
  /** Display name — single-child chains are compressed into "a/b/c". */
  name: string;
  /** Full path key, used to remember the collapsed state across re-renders. */
  path: string;
  dirs: DirNode[];
  files: SessionSummary[];
}

// The tree path for a summary. Rename labels read "old → new"; the file lives
// at the new path.
function treePath(f: SessionSummary): string {
  const label = f.path_label;
  const arrow = label.lastIndexOf(" → ");
  return arrow >= 0 ? label.slice(arrow + 3) : label;
}

function buildTree(files: SessionSummary[]): DirNode {
  const root: DirNode = { name: "", path: "", dirs: [], files: [] };
  for (const f of files) {
    const parts = treePath(f).split("/");
    let node = root;
    for (const part of parts.slice(0, -1)) {
      let child = node.dirs.find((d) => d.name === part);
      if (!child) {
        child = { name: part, path: node.path ? `${node.path}/${part}` : part, dirs: [], files: [] };
        node.dirs.push(child);
      }
      node = child;
    }
    node.files.push(f);
  }
  compress(root);
  return root;
}

// Collapse chains of single-child directories with no files of their own
// ("a" → "b" → "c" becomes one "a/b/c" node), like an IDE project tree.
function compress(node: DirNode) {
  for (const dir of node.dirs) {
    while (dir.dirs.length === 1 && dir.files.length === 0) {
      const only = dir.dirs[0];
      dir.name = `${dir.name}/${only.name}`;
      dir.path = only.path;
      dir.dirs = only.dirs;
      dir.files = only.files;
    }
    compress(dir);
  }
}

function countFiles(node: DirNode): number {
  return node.files.length + node.dirs.reduce((n, d) => n + countFiles(d), 0);
}

// The conflicted-file navigator. Files group into collapsible folders; per-file
// status and whole-file accepts render on the rows. View-switching reuses the
// single MergeEditor via the onSelect callback.
export class FileList {
  private collapsed = new Set<string>();
  private last: { files: SessionSummary[]; activeId: string | null; progress: SessionProgress } | null =
    null;

  constructor(
    private root: HTMLElement,
    private cb: FileListCallbacks
  ) {}

  render(files: SessionSummary[], activeId: string | null, progress: SessionProgress) {
    this.last = { files, activeId, progress };
    const compare = files.length > 0 && files.every((f) => f.change_status);
    const head = compare
      ? `<div class="file-list-head">${progress.total} file(s) changed</div>`
      : `<div class="file-list-head">${progress.resolved_count} of ${progress.total} resolved</div>`;

    const tree = buildTree(files);
    const rows = this.renderDir(tree, activeId, 0);
    this.root.innerHTML = head + `<div class="file-rows">${rows}</div>`;

    this.root.querySelectorAll<HTMLElement>(".tree-dir").forEach((row) => {
      row.addEventListener("click", () => {
        const path = row.dataset.path!;
        if (this.collapsed.has(path)) this.collapsed.delete(path);
        else this.collapsed.add(path);
        if (this.last) this.render(this.last.files, this.last.activeId, this.last.progress);
      });
    });

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

  private renderDir(node: DirNode, activeId: string | null, depth: number): string {
    let html = "";
    for (const dir of node.dirs) {
      const isCollapsed = this.collapsed.has(dir.path);
      html +=
        `<div class="tree-dir" data-path="${escapeHtml(dir.path)}" style="--depth:${depth}">` +
        `<span class="tree-chevron">${isCollapsed ? "▸" : "▾"}</span>` +
        `<span class="tree-name" title="${escapeHtml(dir.path)}">${escapeHtml(dir.name)}</span>` +
        `<span class="tree-count">${countFiles(dir)}</span>` +
        `</div>`;
      if (!isCollapsed) html += this.renderDir(dir, activeId, depth + 1);
    }
    for (const f of node.files) {
      html += this.renderFile(f, activeId, depth);
    }
    return html;
  }

  private renderFile(f: SessionSummary, activeId: string | null, depth: number): string {
    const name = treePath(f).split("/").pop() ?? f.path_label;
    if (f.change_status) {
      const letter = escapeHtml(f.change_status);
      const rowCls = "file-row" + (f.session_id === activeId ? " active" : "");
      return (
        `<div class="${rowCls}" data-id="${f.session_id}" style="--depth:${depth}">` +
        `<span class="file-change file-change-${letter}" title="${letter}">${letter}</span>` +
        `<span class="file-path" title="${escapeHtml(f.path_label)}">${escapeHtml(name)}</span>` +
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
      `<div class="${rowCls}" data-id="${f.session_id}" style="--depth:${depth}">` +
      `<span class="file-status file-status-${f.resolved ? "resolved" : f.kind}" title="${state}"></span>` +
      `<span class="file-path" title="${escapeHtml(f.path_label)}">${escapeHtml(name)}</span>` +
      accept +
      `</div>`
    );
  }
}
