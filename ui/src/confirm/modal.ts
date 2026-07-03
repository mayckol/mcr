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
    const items = unresolved.map((p) => `<li>${escapeHtml(p)}</li>`).join("");
    this.render({
      title: `${unresolved.length} file(s) still unresolved`,
      bodyHtml: `
          <p>These files still have conflicts. Resolved files are already saved and
             will be kept; the rest stay conflicted for a later run.</p>
          <ul class="mcr-confirm-list">${items}</ul>`,
      cancelLabel: "Keep resolving",
      okLabel: "Exit anyway",
      onConfirm,
    });
  }

  confirm(opts: {
    title: string;
    body: string;
    okLabel: string;
    cancelLabel?: string;
    onConfirm: () => void;
  }) {
    this.render({
      title: opts.title,
      bodyHtml: `<p>${escapeHtml(opts.body)}</p>`,
      cancelLabel: opts.cancelLabel ?? "Keep resolving",
      okLabel: opts.okLabel,
      onConfirm: opts.onConfirm,
    });
  }

  private render(opts: {
    title: string;
    bodyHtml: string;
    cancelLabel: string;
    okLabel: string;
    onConfirm: () => void;
  }) {
    this.onConfirm = opts.onConfirm;
    this.overlay.innerHTML = `
      <div class="mcr-modal" role="dialog" aria-label="${escapeHtml(opts.title)}">
        <header class="mcr-modal-head">
          <strong>${escapeHtml(opts.title)}</strong>
          <button class="mcr-modal-close" title="Close">×</button>
        </header>
        <div class="mcr-confirm-body">${opts.bodyHtml}</div>
        <footer class="mcr-modal-foot">
          <button class="mcr-confirm-cancel">${escapeHtml(opts.cancelLabel)}</button>
          <button class="mcr-confirm-ok">${escapeHtml(opts.okLabel)}</button>
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
