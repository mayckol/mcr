function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!
  );
}

// Confirmation modal shown when the user finishes/exits with files still
// unresolved (FR-008/FR-017). Reuses the shared .mcr-modal styling.
export class ExitConfirmModal {
  private overlay: HTMLElement;
  private onConfirm: (() => void) | null = null;

  constructor() {
    this.overlay = document.createElement("div");
    this.overlay.className = "mcr-modal-overlay";
    this.overlay.style.display = "none";
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) this.close();
    });
    document.body.appendChild(this.overlay);
  }

  open(unresolved: string[], onConfirm: () => void) {
    this.onConfirm = onConfirm;
    const items = unresolved.map((p) => `<li>${escapeHtml(p)}</li>`).join("");
    this.overlay.innerHTML = `
      <div class="mcr-modal" role="dialog" aria-label="Unresolved files">
        <header class="mcr-modal-head">
          <strong>${unresolved.length} file(s) still unresolved</strong>
          <button class="mcr-modal-close" title="Close">×</button>
        </header>
        <div class="mcr-confirm-body">
          <p>These files still have conflicts. Resolved files are already saved and
             will be kept; the rest stay conflicted for a later run.</p>
          <ul class="mcr-confirm-list">${items}</ul>
        </div>
        <footer class="mcr-modal-foot">
          <button class="mcr-confirm-cancel">Keep resolving</button>
          <button class="mcr-confirm-ok">Exit anyway</button>
        </footer>
      </div>`;

    this.overlay.querySelector(".mcr-modal-close")!.addEventListener("click", () => this.close());
    this.overlay.querySelector(".mcr-confirm-cancel")!.addEventListener("click", () => this.close());
    this.overlay.querySelector(".mcr-confirm-ok")!.addEventListener("click", () => {
      const cb = this.onConfirm;
      this.close();
      cb?.();
    });
    this.overlay.style.display = "flex";
  }

  close() {
    this.overlay.style.display = "none";
    this.onConfirm = null;
  }
}
