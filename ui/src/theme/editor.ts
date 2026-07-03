import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { tags as t } from "@lezer/highlight";
import type { ThemePalette } from "./themes";

// Editor chrome themed from a palette: gutters, line numbers, cursor, selection
// (the default CodeMirror theme is light — the source of the white gutter strip).
function chrome(p: ThemePalette): Extension {
  return EditorView.theme(
    {
      "&": { color: p.fg, backgroundColor: "transparent", height: "100%" },
      ".cm-scroller": {
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        fontSize: "13px",
        lineHeight: "20px",
      },
      // Fixed, integer line height so every row is identical across panes — banded
      // and plain lines must match exactly or the three panes drift out of alignment.
      ".cm-line": { lineHeight: "20px", padding: "0 4px" },
      ".cm-content": { caretColor: p.fg },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: p.fg },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
        backgroundColor: p.selection,
      },
      ".cm-gutters": {
        backgroundColor: "transparent",
        color: p.gutter,
        border: "none",
      },
      ".cm-gutterElement": { lineHeight: "20px" },
      ".cm-lineNumbers .cm-gutterElement": { color: p.gutter, padding: "0 8px 0 12px" },
      ".cm-activeLineGutter": { backgroundColor: "transparent" },
      ".cm-activeLine": { backgroundColor: "transparent" },
    },
    { dark: p.dark }
  );
}

// Syntax palette from the theme. Distinct hues per token role; comments
// italicised — so merged code reads like it does in the user's editor.
function highlight(p: ThemePalette): HighlightStyle {
  return HighlightStyle.define([
    { tag: [t.comment, t.lineComment, t.blockComment], color: p.comment, fontStyle: "italic" },
    { tag: [t.keyword, t.modifier, t.controlKeyword, t.moduleKeyword], color: p.keyword },
    { tag: [t.operator, t.operatorKeyword, t.compareOperator, t.logicOperator], color: p.operator },
    { tag: [t.string, t.special(t.string), t.regexp], color: p.string },
    { tag: [t.number, t.bool, t.null, t.atom], color: p.number },
    { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: p.func },
    { tag: [t.propertyName, t.attributeName], color: p.property },
    { tag: [t.typeName, t.className, t.namespace], color: p.type },
    { tag: [t.definition(t.variableName), t.variableName, t.local(t.variableName)], color: p.fg },
    { tag: [t.tagName, t.angleBracket], color: p.tag },
    { tag: [t.heading, t.strong], color: p.func, fontWeight: "600" },
    { tag: [t.link, t.url], color: p.operator, textDecoration: "underline" },
    { tag: t.invalid, color: p.danger },
    { tag: [t.meta, t.documentMeta, t.annotation], color: p.meta },
  ]);
}

/** Everything CodeMirror needs from a theme, for one Compartment reconfigure. */
export function editorTheme(p: ThemePalette): Extension {
  return [chrome(p), syntaxHighlighting(highlight(p))];
}
