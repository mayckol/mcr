import type { Category } from "../ipc/types";

// Vendor-neutral category palette (FR-003, FR-017). Semantic, meaning-first
// coloring: green = a change you can safely accept (added / one-sided
// modification), yellow = a conflict you must choose a side for, red = a
// deletion. Values are CSS custom properties so the theme manager retints every
// consumer (line bands, word marks, connector ribbons, gutter) in one pass —
// see theme/manager.ts for the per-theme colors behind the vars.
export const CATEGORY_COLORS: Record<Category, { band: string; word: string; connector: string }> = {
  added: { band: "var(--band-added)", word: "var(--word-added)", connector: "var(--conn-added)" },
  removed: { band: "var(--band-removed)", word: "var(--word-removed)", connector: "var(--conn-removed)" },
  modified: { band: "var(--band-modified)", word: "var(--word-modified)", connector: "var(--conn-modified)" },
  conflicting: {
    band: "var(--band-conflicting)",
    word: "var(--word-conflicting)",
    connector: "var(--conn-conflicting)",
  },
};

export const RESOLVED_DIM = 0.5;
