import { describe, it, expect, vi, beforeEach } from "vitest";
import { applyTheme, storedThemeId } from "../src/theme/manager";
import { THEMES, TOKYO_NIGHT, DAYLIGHT } from "../src/theme/themes";
import { SettingsPanel } from "../src/settings/panel";
import { applyFont, storedFont, DEFAULT_FONT } from "../src/theme/font";

// This jsdom/vitest combo ships no localStorage — back it with a Map.
const store = new Map<string, string>();
Object.defineProperty(globalThis, "localStorage", {
  configurable: true,
  value: {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, String(v)),
    removeItem: (k: string) => void store.delete(k),
    clear: () => store.clear(),
  },
});

beforeEach(() => {
  document.body.innerHTML = "";
  document.documentElement.removeAttribute("style");
  store.clear();
});

describe("theme manager", () => {
  it("defaults to Tokyo Night", () => {
    expect(storedThemeId()).toBe("tokyo-night");
  });

  it("applyTheme paints CSS vars, sets color-scheme, and persists", () => {
    const p = applyTheme("daylight");
    expect(p.id).toBe("daylight");
    const style = document.documentElement.style;
    expect(style.getPropertyValue("--bg")).toBe(DAYLIGHT.bg);
    expect(style.getPropertyValue("--accent")).toBe(DAYLIGHT.accent);
    expect(style.getPropertyValue("--band-conflicting")).toBe(DAYLIGHT.conflicting.band);
    expect(style.colorScheme).toBe("light");
    expect(storedThemeId()).toBe("daylight");
  });

  it("unknown stored id falls back to Tokyo Night", () => {
    localStorage.setItem("mcr.theme", "not-a-theme");
    expect(storedThemeId()).toBe("tokyo-night");
    const p = applyTheme(storedThemeId());
    expect(document.documentElement.style.getPropertyValue("--bg")).toBe(TOKYO_NIGHT.bg);
    expect(p.dark).toBe(true);
  });
});

describe("SettingsPanel (Appearance)", () => {
  it("lists every theme with the active one marked", () => {
    applyTheme("tokyo-storm");
    const panel = new SettingsPanel(() => {}, () => {});
    panel.open();
    const cards = document.querySelectorAll(".th-card");
    expect(cards.length).toBe(THEMES.length);
    const active = document.querySelector(".th-card.active") as HTMLElement;
    expect(active.dataset.theme).toBe("tokyo-storm");
  });

  it("clicking a card reports the theme id and moves the active mark", () => {
    const onTheme = vi.fn((id) => applyTheme(id));
    const panel = new SettingsPanel(onTheme, () => {});
    panel.open();
    (document.querySelector('.th-card[data-theme="ember"]') as HTMLElement).click();
    expect(onTheme).toHaveBeenCalledWith("ember");
    expect((document.querySelector(".th-card.active") as HTMLElement).dataset.theme).toBe("ember");
  });

  it("close hides the overlay", () => {
    const panel = new SettingsPanel(() => {}, () => {});
    panel.open();
    (document.querySelector(".mcr-settings .mcr-modal-close") as HTMLElement).click();
    expect((document.querySelector(".mcr-modal-overlay") as HTMLElement).style.display).toBe("none");
  });
});

describe("font manager", () => {
  it("defaults to SF Mono, Regular, 13px", () => {
    expect(storedFont()).toEqual(DEFAULT_FONT);
  });

  it("applyFont normalizes, clamps size, and persists", () => {
    const saved = applyFont({ family: "fira-code", weight: "bold", size: 99 });
    expect(saved).toEqual({ family: "fira-code", weight: "bold", size: 32 });
    expect(storedFont()).toEqual({ family: "fira-code", weight: "bold", size: 32 });
  });

  it("unknown stored values fall back to defaults", () => {
    localStorage.setItem("mcr.font", JSON.stringify({ family: "nope", weight: "nope", size: 3 }));
    const f = storedFont();
    expect(f.family).toBe(DEFAULT_FONT.family);
    expect(f.weight).toBe(DEFAULT_FONT.weight);
    expect(f.size).toBe(8);
  });
});

describe("SettingsPanel (Editor font)", () => {
  it("reports the chosen font on change", () => {
    const onFont = vi.fn();
    const panel = new SettingsPanel(() => {}, onFont);
    panel.open();
    const family = document.querySelector('[data-font="family"]') as HTMLSelectElement;
    family.value = "menlo";
    family.dispatchEvent(new Event("change"));
    expect(onFont).toHaveBeenCalledWith({ family: "menlo", weight: "regular", size: 13 });
  });
});
