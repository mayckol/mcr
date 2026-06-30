import type { Category } from "../ipc/types";

// Vendor-neutral category palette (FR-003, FR-017). Distinct, consistent hues
// for added / removed / modified / conflicting; no names borrowed from any IDE.
// Tokyo Night accents.
export const CATEGORY_COLORS: Record<Category, { band: string; word: string; connector: string }> = {
  added: { band: "rgba(158, 206, 106, 0.20)", word: "rgba(158, 206, 106, 0.42)", connector: "#9ece6a" },
  removed: { band: "rgba(247, 118, 142, 0.20)", word: "rgba(247, 118, 142, 0.42)", connector: "#f7768e" },
  modified: { band: "rgba(122, 162, 247, 0.20)", word: "rgba(122, 162, 247, 0.42)", connector: "#7aa2f7" },
  conflicting: { band: "rgba(224, 175, 104, 0.26)", word: "rgba(224, 175, 104, 0.48)", connector: "#e0af68" },
};

export const RESOLVED_DIM = 0.5;
