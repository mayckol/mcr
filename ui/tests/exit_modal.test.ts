import { describe, it, expect, vi, beforeEach } from "vitest";
import { ExitConfirmModal } from "../src/confirm/modal";

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("ExitConfirmModal (FR-008)", () => {
  it("lists every unresolved file when opened", () => {
    const modal = new ExitConfirmModal();
    modal.open(["a.txt", "b.txt"], () => {});
    const overlay = document.querySelector(".mcr-modal-overlay") as HTMLElement;
    expect(overlay.style.display).toBe("flex");
    expect(overlay.querySelector(".mcr-modal-head")!.textContent).toContain("2 file(s)");
    const items = [...overlay.querySelectorAll(".mcr-confirm-list li")].map((li) => li.textContent);
    expect(items).toEqual(["a.txt", "b.txt"]);
  });

  it("cancel closes without confirming", () => {
    const modal = new ExitConfirmModal();
    const onConfirm = vi.fn();
    modal.open(["a.txt"], onConfirm);
    (document.querySelector(".mcr-confirm-cancel") as HTMLElement).click();
    expect(onConfirm).not.toHaveBeenCalled();
    expect((document.querySelector(".mcr-modal-overlay") as HTMLElement).style.display).toBe("none");
  });

  it("'Exit anyway' confirms and closes", () => {
    const modal = new ExitConfirmModal();
    const onConfirm = vi.fn();
    modal.open(["a.txt"], onConfirm);
    (document.querySelector(".mcr-confirm-ok") as HTMLElement).click();
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect((document.querySelector(".mcr-modal-overlay") as HTMLElement).style.display).toBe("none");
  });
});
