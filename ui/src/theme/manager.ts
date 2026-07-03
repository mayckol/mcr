import { DEFAULT_THEME, themeById, type ThemeId, type ThemePalette } from "./themes";

const STORAGE_KEY = "mcr.theme";

// Everything the stylesheet consumes. Decorations and connectors also read the
// --band/--word vars (via var() color strings), so one reapply retints the whole
// app — no per-consumer plumbing.
function cssVars(p: ThemePalette): Record<string, string> {
  return {
    "--bg": p.bg,
    "--bg-soft": p.bgSoft,
    "--bg-elev": p.bgElev,
    "--border": p.border,
    "--fg": p.fg,
    "--fg-dim": p.fgDim,
    "--accent": p.accent,
    "--on-accent": p.onAccent,
    "--ok": p.ok,
    "--warn": p.warn,
    "--danger": p.danger,
    "--info": p.info,
    "--special": p.special,
    "--pane-shadow": p.paneShadow,
    "--brand-chip": p.dark ? "transparent" : "#16161e",
    "--scroll-thumb": p.scrollThumb,
    "--scroll-thumb-hover": p.scrollThumbHover,
    "--scroll-thumb-active": p.accent,
    "--band-added": p.added.band,
    "--band-removed": p.removed.band,
    "--band-modified": p.modified.band,
    "--band-conflicting": p.conflicting.band,
    "--word-added": p.added.word,
    "--word-removed": p.removed.word,
    "--word-modified": p.modified.word,
    "--word-conflicting": p.conflicting.word,
    "--conn-added": p.added.connector,
    "--conn-removed": p.removed.connector,
    "--conn-modified": p.modified.connector,
    "--conn-conflicting": p.conflicting.connector,
  };
}

export function storedThemeId(): ThemeId {
  try {
    return themeById(localStorage.getItem(STORAGE_KEY) ?? DEFAULT_THEME).id;
  } catch {
    return DEFAULT_THEME;
  }
}

/** Paint a palette onto the document and remember the choice. */
export function applyTheme(id: ThemeId, onPalette?: (p: ThemePalette) => void): ThemePalette {
  const p = themeById(id);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(cssVars(p))) root.style.setProperty(k, v);
  root.style.colorScheme = p.dark ? "dark" : "light";
  try {
    localStorage.setItem(STORAGE_KEY, p.id);
  } catch {
    // Storage unavailable (private mode) — theme still applies for this run.
  }
  onPalette?.(p);
  return p;
}
