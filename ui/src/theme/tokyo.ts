import { EditorView } from "@codemirror/view";

// Tokyo Night palette.
export const TOKYO = {
  bg: "#1a1b26",
  bgDark: "#16161e",
  bgHighlight: "#292e42",
  border: "#3b4261",
  fg: "#c0caf5",
  fgGutter: "#545c7e",
  comment: "#565f89",
  selection: "#283457",
  blue: "#7aa2f7",
  green: "#9ece6a",
  red: "#f7768e",
  yellow: "#e0af68",
  orange: "#ff9e64",
  // Keyword purple matches the enkia "Tokyo Night" editor theme (#9d7cd8), not the
  // lighter magenta (#bb9af7) some ports use — kept separately for markup accents.
  purple: "#9d7cd8",
  magenta: "#bb9af7",
  cyan: "#7dcfff",
  teal: "#2ac3de",
};

// Dark editor theme so gutters, line numbers, cursor, and selection are themed
// (the default CodeMirror theme is light — the source of the white gutter strip).
export const tokyoNight = EditorView.theme(
  {
    "&": { color: TOKYO.fg, backgroundColor: "transparent", height: "100%" },
    ".cm-scroller": {
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
      fontSize: "13px",
      lineHeight: "20px",
    },
    // Fixed, integer line height so every row is identical across panes — banded
    // and plain lines must match exactly or the three panes drift out of alignment.
    ".cm-line": { lineHeight: "20px", padding: "0 4px" },
    ".cm-content": { caretColor: TOKYO.fg },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: TOKYO.fg },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
      backgroundColor: TOKYO.selection,
    },
    ".cm-gutters": {
      backgroundColor: "transparent",
      color: TOKYO.fgGutter,
      border: "none",
    },
    ".cm-gutterElement": { lineHeight: "20px" },
    ".cm-lineNumbers .cm-gutterElement": { color: TOKYO.fgGutter, padding: "0 8px 0 12px" },
    ".cm-activeLineGutter": { backgroundColor: "transparent" },
    ".cm-activeLine": { backgroundColor: "transparent" },
  },
  { dark: true }
);
