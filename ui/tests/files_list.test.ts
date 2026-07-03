import { describe, it, expect, vi } from "vitest";
import { FileList } from "../src/files/list";
import type { SessionSummary, SessionProgress } from "../src/ipc/types";

const progress: SessionProgress = {
  total: 3,
  resolved_count: 1,
  remaining_conflicts: 2,
  all_resolved: false,
};

const files: SessionSummary[] = [
  { session_id: "session-1", path_label: "a.txt", kind: "text", resolved: false, remaining_conflicts: 1 },
  { session_id: "session-2", path_label: "b.txt", kind: "text", resolved: true, remaining_conflicts: 0 },
  { session_id: "session-3", path_label: "img.png", kind: "binary", resolved: false, remaining_conflicts: 0 },
];

function mount() {
  document.body.innerHTML = '<aside id="fl"></aside>';
  const root = document.getElementById("fl")!;
  const onSelect = vi.fn();
  const onAccept = vi.fn();
  const list = new FileList(root, { onSelect, onAccept });
  return { root, list, onSelect, onAccept };
}

describe("FileList (US1)", () => {
  it("renders one row per file in stable order with path + status", () => {
    const { root, list } = mount();
    list.render(files, "session-1", progress);
    const rows = root.querySelectorAll(".file-row");
    expect(rows.length).toBe(3);
    expect([...rows].map((r) => (r as HTMLElement).dataset.id)).toEqual([
      "session-1",
      "session-2",
      "session-3",
    ]);
    expect(root.querySelector(".file-list-head")!.textContent).toContain("1 of 3 resolved");
    // active + resolved markers
    expect((rows[0] as HTMLElement).classList.contains("active")).toBe(true);
    expect((rows[1] as HTMLElement).classList.contains("is-resolved")).toBe(true);
  });

  it("keeps input order even when status changes (FR-012)", () => {
    const { root, list } = mount();
    const flipped = files.map((f) => ({ ...f, resolved: !f.resolved }));
    list.render(flipped, null, progress);
    expect([...root.querySelectorAll(".file-row")].map((r) => (r as HTMLElement).dataset.id)).toEqual([
      "session-1",
      "session-2",
      "session-3",
    ]);
  });

  it("selecting a row calls onSelect", () => {
    const { root, list, onSelect } = mount();
    list.render(files, null, progress);
    (root.querySelector('.file-row[data-id="session-1"]') as HTMLElement).click();
    expect(onSelect).toHaveBeenCalledWith("session-1");
  });

  it("accept buttons call onAccept and do not also select (FR-009)", () => {
    const { root, list, onSelect, onAccept } = mount();
    list.render(files, null, progress);
    (root.querySelector('.file-row[data-id="session-1"] .accept-local') as HTMLElement).click();
    expect(onAccept).toHaveBeenCalledWith("session-1", "local");
    (root.querySelector('.file-row[data-id="session-3"] .accept-incoming') as HTMLElement).click();
    expect(onAccept).toHaveBeenCalledWith("session-3", "incoming");
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("resolved files expose no accept controls", () => {
    const { root, list } = mount();
    list.render(files, null, progress);
    expect(root.querySelector('.file-row[data-id="session-2"] .file-accept')).toBeNull();
  });
});

describe("FileList (compare mode)", () => {
  const compareFiles: SessionSummary[] = [
    { session_id: "session-1", path_label: "new.ts", kind: "text", resolved: false, remaining_conflicts: 0, change_status: "A" },
    { session_id: "session-2", path_label: "mod.ts", kind: "text", resolved: false, remaining_conflicts: 1, change_status: "M" },
    { session_id: "session-3", path_label: "old.ts", kind: "text", resolved: false, remaining_conflicts: 0, change_status: "D" },
  ];

  it("renders name-status badges and no accept buttons", () => {
    const { root, list } = mount();
    list.render(compareFiles, "session-2", progress);
    const badges = [...root.querySelectorAll(".file-change")].map((b) => b.textContent);
    expect(badges).toEqual(["A", "M", "D"]);
    expect(root.querySelector(".file-change-A")).not.toBeNull();
    expect(root.querySelector(".file-accept")).toBeNull();
    expect(root.querySelector(".file-status")).toBeNull();
    expect(root.querySelector(".file-list-head")!.textContent).toContain("3 file(s) changed");
  });

  it("row click selects the compared file", () => {
    const { root, list, onSelect } = mount();
    list.render(compareFiles, null, progress);
    (root.querySelector('.file-row[data-id="session-3"]') as HTMLElement).click();
    expect(onSelect).toHaveBeenCalledWith("session-3");
  });
});

describe("FileList (folder tree)", () => {
  const nested: SessionSummary[] = [
    { session_id: "s1", path_label: "internal/entity/interface.go", kind: "text", resolved: false, remaining_conflicts: 1 },
    { session_id: "s2", path_label: "internal/entity/detail.go", kind: "text", resolved: false, remaining_conflicts: 1 },
    { session_id: "s3", path_label: "internal/web/routes.go", kind: "text", resolved: false, remaining_conflicts: 0 },
    { session_id: "s4", path_label: "root.txt", kind: "text", resolved: false, remaining_conflicts: 0 },
  ];

  it("groups files under collapsible folders with counts", () => {
    const { root, list } = mount();
    list.render(nested, null, progress);
    const dirs = [...root.querySelectorAll(".tree-dir .tree-name")].map((d) => d.textContent);
    expect(dirs).toEqual(["internal", "entity", "web"]);
    const counts = [...root.querySelectorAll(".tree-dir .tree-count")].map((c) => c.textContent);
    expect(counts).toEqual(["3", "2", "1"]);
    // File rows show basenames; full path stays in the tooltip.
    const names = [...root.querySelectorAll(".file-row .file-path")].map((f) => f.textContent);
    expect(names).toEqual(["interface.go", "detail.go", "routes.go", "root.txt"]);
  });

  it("compresses single-child directory chains", () => {
    const { root, list } = mount();
    list.render(
      [{ session_id: "s1", path_label: "a/b/c/deep.ts", kind: "text", resolved: false, remaining_conflicts: 0 }],
      null,
      progress
    );
    const dirs = [...root.querySelectorAll(".tree-dir .tree-name")].map((d) => d.textContent);
    expect(dirs).toEqual(["a/b/c"]);
  });

  it("clicking a folder collapses and re-expands its files", () => {
    const { root, list } = mount();
    list.render(nested, null, progress);
    (root.querySelector('.tree-dir[data-path="internal/entity"]') as HTMLElement).click();
    expect(root.querySelector('.file-row[data-id="s1"]')).toBeNull();
    expect(root.querySelector('.file-row[data-id="s3"]')).not.toBeNull();
    (root.querySelector('.tree-dir[data-path="internal/entity"]') as HTMLElement).click();
    expect(root.querySelector('.file-row[data-id="s1"]')).not.toBeNull();
  });
});
