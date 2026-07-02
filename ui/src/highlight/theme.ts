import type { Category } from "../ipc/types";

// Vendor-neutral category palette (FR-003, FR-017). Distinct, consistent hues
// for added / removed / modified / conflicting; no names borrowed from any IDE.
// Tokyo Night accents.
// Semantic, meaning-first coloring: green = a change you can safely accept
// (added / one-sided modification), yellow = a conflict you must choose a side for,
// red = a deletion. One flat hue per row — the gutter is driven from the same
// values (see main.ts → --band-* vars) so a row never stripes gutter-vs-content.
export const CATEGORY_COLORS: Record<Category, { band: string; word: string; connector: string }> = {
  added: { band: "rgba(158, 206, 106, 0.18)", word: "rgba(158, 206, 106, 0.36)", connector: "#9ece6a" },
  removed: { band: "rgba(247, 118, 142, 0.18)", word: "rgba(247, 118, 142, 0.36)", connector: "#f7768e" },
  modified: { band: "rgba(158, 206, 106, 0.18)", word: "rgba(158, 206, 106, 0.36)", connector: "#9ece6a" },
  conflicting: { band: "rgba(224, 175, 104, 0.18)", word: "rgba(224, 175, 104, 0.36)", connector: "#e0af68" },
};

export const RESOLVED_DIM = 0.5;
