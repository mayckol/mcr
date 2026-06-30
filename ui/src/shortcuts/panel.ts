import {
  Shortcuts,
  ACTION_LABELS,
  type Action,
  chordFromEvent,
  displayChord,
} from "./keymap";

// Modal to view and rebind shortcuts (the "shortcut session"). Click a chord,
// press the new combination; Esc cancels a capture, Reset restores defaults.
export class ShortcutsPanel {
  private overlay: HTMLElement;

  constructor(private shortcuts: Shortcuts) {
    this.overlay = document.createElement("div");
    this.overlay.className = "mcr-modal-overlay";
    this.overlay.style.display = "none";
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) this.close();
    });
    document.body.appendChild(this.overlay);
  }

  open() {
    this.render();
    this.overlay.style.display = "flex";
  }

  close() {
    this.cancelCapture();
    this.overlay.style.display = "none";
  }

  private render() {
    const bindings = this.shortcuts.all();
    const rows = (Object.keys(ACTION_LABELS) as Action[])
      .map((action) => {
        const conflict = this.isConflict(action, bindings[action]);
        return `
          <div class="mcr-kb-row">
            <span class="mcr-kb-label">${ACTION_LABELS[action]}</span>
            <button class="mcr-kb-chord${conflict ? " mcr-kb-conflict" : ""}" data-action="${action}">
              ${displayChord(bindings[action])}
            </button>
          </div>`;
      })
      .join("");

    this.overlay.innerHTML = `
      <div class="mcr-modal" role="dialog" aria-label="Keyboard shortcuts">
        <header class="mcr-modal-head">
          <strong>Keyboard Shortcuts</strong>
          <button class="mcr-modal-close" title="Close">×</button>
        </header>
        <div class="mcr-kb-list">${rows}</div>
        <footer class="mcr-modal-foot">
          <span class="mcr-kb-hint">Click a shortcut, then press the new keys</span>
          <button class="mcr-kb-reset">Reset to defaults</button>
        </footer>
      </div>`;

    this.overlay.querySelector(".mcr-modal-close")!.addEventListener("click", () => this.close());
    this.overlay.querySelector(".mcr-kb-reset")!.addEventListener("click", () => {
      this.shortcuts.reset();
      this.render();
    });
    this.overlay.querySelectorAll<HTMLButtonElement>(".mcr-kb-chord").forEach((btn) => {
      btn.addEventListener("click", () => this.beginCapture(btn.dataset.action as Action, btn));
    });
  }

  private isConflict(action: Action, chord: string): boolean {
    const all = this.shortcuts.all();
    return (Object.keys(all) as Action[]).some((a) => a !== action && all[a] === chord);
  }

  private beginCapture(action: Action, btn: HTMLButtonElement) {
    this.cancelCapture();
    this.shortcuts.suspend(true);
    btn.classList.add("mcr-kb-capturing");
    btn.textContent = "Press keys…";

    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        this.cancelCapture();
        this.render();
        return;
      }
      const chord = chordFromEvent(e);
      if (!chord) return; // lone modifier — keep waiting
      this.shortcuts.set(action, chord);
      this.cancelCapture();
      this.render();
    };
    this.captureHandler = onKey;
    window.addEventListener("keydown", onKey, true);
  }

  private captureHandler: ((e: KeyboardEvent) => void) | null = null;

  private cancelCapture() {
    if (this.captureHandler) {
      window.removeEventListener("keydown", this.captureHandler, true);
      this.captureHandler = null;
    }
    this.shortcuts.suspend(false);
  }
}
