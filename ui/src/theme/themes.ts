// Theme registry. Every color the app uses — chrome, editor, syntax, change
// bands — comes from one ThemePalette, applied as CSS custom properties plus a
// CodeMirror extension. Adding a theme = adding one palette object here.

export type ThemeId = "tokyo-night" | "tokyo-storm" | "daylight" | "ember";

export interface CategoryTint {
  /** Whole-line band + connector ribbon fill. */
  band: string;
  /** Intra-line word-diff highlight. */
  word: string;
  /** Connector hue (kept for callers that want a solid line color). */
  connector: string;
}

export interface ThemePalette {
  id: ThemeId;
  label: string;
  /** Drives CodeMirror's dark flag and the webview's color-scheme. */
  dark: boolean;

  // Chrome
  bg: string;
  bgSoft: string;
  bgElev: string;
  border: string;
  fg: string;
  fgDim: string;
  accent: string;
  /** Text color on accent-filled controls. */
  onAccent: string;
  selection: string;
  scrollThumb: string;
  scrollThumbHover: string;
  /** Soft shadow under panes/modals — themes tune it for light vs dark. */
  paneShadow: string;

  // Semantic state hues (file list dots, badges, warnings).
  ok: string;
  warn: string;
  danger: string;
  info: string;
  special: string;

  // Syntax
  comment: string;
  keyword: string;
  operator: string;
  string: string;
  number: string;
  func: string;
  property: string;
  type: string;
  tag: string;
  meta: string;
  gutter: string;

  // Change bands per category.
  added: CategoryTint;
  removed: CategoryTint;
  modified: CategoryTint;
  conflicting: CategoryTint;
}

const tint = (rgb: string, band: number, word: number, connector: string): CategoryTint => ({
  band: `rgba(${rgb}, ${band})`,
  word: `rgba(${rgb}, ${word})`,
  connector,
});

export const TOKYO_NIGHT: ThemePalette = {
  id: "tokyo-night",
  label: "Tokyo Night",
  dark: true,
  bg: "#1a1b26",
  bgSoft: "#16161e",
  bgElev: "#292e42",
  border: "#3b4261",
  fg: "#c0caf5",
  fgDim: "#565f89",
  accent: "#7aa2f7",
  onAccent: "#0d1117",
  selection: "#283457",
  scrollThumb: "#2c3252",
  scrollThumbHover: "#3d4673",
  paneShadow: "0 2px 10px rgba(0, 0, 0, 0.35)",
  ok: "#9ece6a",
  warn: "#ffc777",
  danger: "#f7768e",
  info: "#7dcfff",
  special: "#bb9af7",
  comment: "#565f89",
  keyword: "#9d7cd8",
  operator: "#7dcfff",
  string: "#9ece6a",
  number: "#ff9e64",
  func: "#7aa2f7",
  property: "#7dcfff",
  type: "#2ac3de",
  tag: "#f7768e",
  meta: "#545c7e",
  gutter: "#545c7e",
  added: tint("125, 207, 255", 0.15, 0.32, "#7dcfff"),
  removed: tint("247, 118, 142", 0.18, 0.36, "#f7768e"),
  modified: tint("125, 207, 255", 0.15, 0.32, "#7dcfff"),
  conflicting: tint("255, 199, 119", 0.16, 0.34, "#ffc777"),
};

export const TOKYO_STORM: ThemePalette = {
  ...TOKYO_NIGHT,
  id: "tokyo-storm",
  label: "Tokyo Storm",
  bg: "#24283b",
  bgSoft: "#1f2335",
  bgElev: "#343a55",
  border: "#414868",
  fgDim: "#606a92",
  selection: "#2e3c64",
  scrollThumb: "#363d5c",
  scrollThumbHover: "#485283",
};

export const DAYLIGHT: ThemePalette = {
  id: "daylight",
  label: "Daylight",
  dark: false,
  bg: "#e1e2e7",
  bgSoft: "#d5d6db",
  bgElev: "#eef0f6",
  border: "#b3b8d1",
  fg: "#343b58",
  fgDim: "#848cb5",
  accent: "#2e7de9",
  onAccent: "#ffffff",
  selection: "#b7c1e3",
  scrollThumb: "#bcc0d6",
  scrollThumbHover: "#a4aac9",
  paneShadow: "0 2px 8px rgba(52, 59, 88, 0.14)",
  ok: "#587539",
  warn: "#8c6c3e",
  danger: "#f52a65",
  info: "#007197",
  special: "#7847bd",
  comment: "#848cb5",
  keyword: "#7847bd",
  operator: "#007197",
  string: "#587539",
  number: "#b15c00",
  func: "#2e7de9",
  property: "#007197",
  type: "#118c74",
  tag: "#f52a65",
  meta: "#9699a3",
  gutter: "#9699a3",
  added: tint("88, 117, 57", 0.16, 0.3, "#587539"),
  removed: tint("245, 42, 101", 0.12, 0.24, "#f52a65"),
  modified: tint("88, 117, 57", 0.16, 0.3, "#587539"),
  conflicting: tint("140, 108, 62", 0.2, 0.38, "#8c6c3e"),
};

export const EMBER: ThemePalette = {
  id: "ember",
  label: "Ember",
  dark: true,
  bg: "#1c1917",
  bgSoft: "#151311",
  bgElev: "#2b2521",
  border: "#41362d",
  fg: "#e7ddd0",
  fgDim: "#8d7e6f",
  accent: "#ff9e64",
  onAccent: "#1c1512",
  selection: "#3a3129",
  scrollThumb: "#332b25",
  scrollThumbHover: "#4a3e35",
  paneShadow: "0 2px 10px rgba(0, 0, 0, 0.4)",
  ok: "#9bbf65",
  warn: "#eab86b",
  danger: "#e0687a",
  info: "#56c2b0",
  special: "#c79bf7",
  comment: "#8d7e6f",
  keyword: "#e0687a",
  operator: "#d9a35c",
  string: "#9bbf65",
  number: "#ff9e64",
  func: "#eab86b",
  property: "#d9a35c",
  type: "#56c2b0",
  tag: "#e0687a",
  meta: "#6f6255",
  gutter: "#6f6255",
  added: tint("155, 191, 101", 0.16, 0.32, "#9bbf65"),
  removed: tint("224, 104, 122", 0.16, 0.32, "#e0687a"),
  modified: tint("155, 191, 101", 0.16, 0.32, "#9bbf65"),
  conflicting: tint("234, 184, 107", 0.18, 0.36, "#eab86b"),
};

export const THEMES: readonly ThemePalette[] = [TOKYO_NIGHT, TOKYO_STORM, DAYLIGHT, EMBER];

export const DEFAULT_THEME: ThemeId = "tokyo-night";

export function themeById(id: string | null | undefined): ThemePalette {
  return THEMES.find((t) => t.id === id) ?? TOKYO_NIGHT;
}
