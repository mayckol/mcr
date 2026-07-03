import { THEMES, type ThemeId, type ThemePalette } from "../theme/themes";
import { storedThemeId } from "../theme/manager";

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!
  );
}

// A theme card's preview: the palette painted as a miniature three-pane diff,
// so picking a theme means seeing exactly what it does to the thing this app is.
function preview(p: ThemePalette): string {
  const line = (color: string, w: number) =>
    `<span class="th-line" style="background:${color};width:${w}%"></span>`;
  const pane = (rows: string) => `<span class="th-pane" style="background:${p.bg}">${rows}</span>`;
  return (
    `<span class="th-preview" style="background:${p.bgSoft};border-color:${p.border}">` +
    pane(line(p.added.word, 70) + line(p.fgDim, 45) + line(p.string, 60)) +
    pane(line(p.conflicting.word, 55) + line(p.accent, 65) + line(p.fgDim, 40)) +
    pane(line(p.removed.word, 60) + line(p.fgDim, 50) + line(p.keyword, 45)) +
    `</span>`
  );
}

/** Settings modal. One section for now — Appearance — themed live on click. */
export class SettingsPanel {
  private overlay: HTMLElement;

  constructor(private onTheme: (id: ThemeId) => void) {
    this.overlay = document.createElement("div");
    this.overlay.className = "mcr-modal-overlay";
    this.overlay.style.display = "none";
    this.overlay.addEventListener("click", (e) => {
      if (e.target === this.overlay) this.close();
    });
    document.body.appendChild(this.overlay);
  }

  open() {
    this.renderInto();
    this.overlay.style.display = "flex";
  }

  close() {
    this.overlay.style.display = "none";
  }

  private renderInto() {
    const active = storedThemeId();
    const cards = THEMES.map(
      (t) =>
        `<button class="th-card${t.id === active ? " active" : ""}" data-theme="${t.id}">` +
        preview(t) +
        `<span class="th-name">${escapeHtml(t.label)}</span>` +
        `</button>`
    ).join("");

    this.overlay.innerHTML = `
      <div class="mcr-modal mcr-settings" role="dialog" aria-label="Settings">
        <header class="mcr-modal-head">
          <strong>Settings</strong>
          <button class="mcr-modal-close" title="Close">×</button>
        </header>
        <div class="mcr-settings-body">
          <div class="mcr-settings-section">Appearance</div>
          <p class="mcr-settings-hint">Theme applies immediately and is remembered.</p>
          <div class="th-grid">${cards}</div>
        </div>
      </div>`;

    this.overlay.querySelector(".mcr-modal-close")!.addEventListener("click", () => this.close());
    this.overlay.querySelectorAll<HTMLElement>(".th-card").forEach((card) => {
      card.addEventListener("click", () => {
        this.onTheme(card.dataset.theme as ThemeId);
        this.renderInto(); // re-render so the active outline follows the pick
      });
    });
  }
}
