import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

export interface FontOption {
  id: string;
  label: string;
  stack: string;
}

export interface WeightOption {
  id: string;
  label: string;
  value: string;
}

export interface FontSettings {
  family: string;
  weight: string;
  size: number;
}

// None of these are bundled — each stack degrades to the system monospace when
// the named face is absent, so a pick is always safe even off a fresh machine.
export const FONT_FAMILIES: FontOption[] = [
  { id: "sf-mono", label: "SF Mono", stack: `"SF Mono", SFMono-Regular, ui-monospace, Menlo, monospace` },
  { id: "menlo", label: "Menlo", stack: `Menlo, Monaco, ui-monospace, monospace` },
  { id: "fira-code", label: "Fira Code", stack: `"Fira Code", ui-monospace, SFMono-Regular, Menlo, monospace` },
  { id: "cascadia-code", label: "Cascadia Code", stack: `"Cascadia Code", ui-monospace, SFMono-Regular, Menlo, monospace` },
  { id: "source-code-pro", label: "Source Code Pro", stack: `"Source Code Pro", ui-monospace, SFMono-Regular, Menlo, monospace` },
  { id: "ibm-plex-mono", label: "IBM Plex Mono", stack: `"IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, monospace` },
  { id: "system", label: "System monospace", stack: `ui-monospace, SFMono-Regular, Menlo, monospace` },
];

export const FONT_WEIGHTS: WeightOption[] = [
  { id: "light", label: "Light", value: "300" },
  { id: "regular", label: "Regular", value: "400" },
  { id: "medium", label: "Medium", value: "500" },
  { id: "semibold", label: "Semibold", value: "600" },
  { id: "bold", label: "Bold", value: "700" },
];

export const DEFAULT_FONT: FontSettings = { family: "sf-mono", weight: "regular", size: 13 };
const MIN_SIZE = 8;
const MAX_SIZE = 32;
const STORAGE_KEY = "mcr.font";

export function familyById(id: string): FontOption {
  return FONT_FAMILIES.find((f) => f.id === id) ?? FONT_FAMILIES[0];
}

export function weightById(id: string): WeightOption {
  return FONT_WEIGHTS.find((w) => w.id === id) ?? FONT_WEIGHTS[1];
}

function clampSize(n: number): number {
  if (!Number.isFinite(n)) return DEFAULT_FONT.size;
  return Math.min(MAX_SIZE, Math.max(MIN_SIZE, Math.round(n)));
}

export function storedFont(): FontSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_FONT };
    const parsed = JSON.parse(raw) as Partial<FontSettings>;
    return {
      family: familyById(parsed.family ?? DEFAULT_FONT.family).id,
      weight: weightById(parsed.weight ?? DEFAULT_FONT.weight).id,
      size: clampSize(parsed.size ?? DEFAULT_FONT.size),
    };
  } catch {
    return { ...DEFAULT_FONT };
  }
}

export function applyFont(settings: FontSettings): FontSettings {
  const normalized: FontSettings = {
    family: familyById(settings.family).id,
    weight: weightById(settings.weight).id,
    size: clampSize(settings.size),
  };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(normalized));
  } catch {
    // Storage unavailable (private mode) — font still applies for this run.
  }
  return normalized;
}

/**
 * Font-driven editor styling as its own compartment: family, weight, size, and
 * the line height derived from size. Line height must stay an integer and match
 * across all three panes, or banded and plain rows drift out of alignment.
 */
export function fontExtension(settings: FontSettings): Extension {
  const { stack } = familyById(settings.family);
  const weight = weightById(settings.weight).value;
  const size = clampSize(settings.size);
  const lineHeight = Math.round(size * 1.55);
  return EditorView.theme({
    ".cm-scroller": {
      fontFamily: stack,
      fontSize: `${size}px`,
      lineHeight: `${lineHeight}px`,
    },
    ".cm-content": { fontWeight: weight },
    ".cm-line": { lineHeight: `${lineHeight}px` },
    ".cm-gutterElement": { lineHeight: `${lineHeight}px` },
  });
}
