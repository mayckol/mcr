import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import type { StreamParser } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { tags as t } from "@lezer/highlight";
import { TOKYO } from "../theme/tokyo";

import { javascript, json, typescript } from "@codemirror/legacy-modes/mode/javascript";
import { c, cpp, java, csharp, kotlin, scala, objectiveC, dart } from "@codemirror/legacy-modes/mode/clike";
import { python } from "@codemirror/legacy-modes/mode/python";
import { rust } from "@codemirror/legacy-modes/mode/rust";
import { go } from "@codemirror/legacy-modes/mode/go";
import { ruby } from "@codemirror/legacy-modes/mode/ruby";
import { swift } from "@codemirror/legacy-modes/mode/swift";
import { css, sCSS, less } from "@codemirror/legacy-modes/mode/css";
import { xml, html } from "@codemirror/legacy-modes/mode/xml";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { yaml } from "@codemirror/legacy-modes/mode/yaml";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { perl } from "@codemirror/legacy-modes/mode/perl";
import { haskell } from "@codemirror/legacy-modes/mode/haskell";
import { dockerFile } from "@codemirror/legacy-modes/mode/dockerfile";
import { properties } from "@codemirror/legacy-modes/mode/properties";
import { standardSQL } from "@codemirror/legacy-modes/mode/sql";
import { diff } from "@codemirror/legacy-modes/mode/diff";

// Tokyo Night syntax palette. Distinct hues for each token role; comments
// italicised, keywords in purple, strings in green — the canonical Tokyo Night
// reading so merged code looks like it does in the user's editor.
const tokyoHighlight = HighlightStyle.define([
  { tag: [t.comment, t.lineComment, t.blockComment], color: TOKYO.comment, fontStyle: "italic" },
  { tag: [t.keyword, t.modifier, t.controlKeyword, t.moduleKeyword], color: TOKYO.purple },
  { tag: [t.operator, t.operatorKeyword, t.compareOperator, t.logicOperator], color: TOKYO.cyan },
  { tag: [t.string, t.special(t.string), t.regexp], color: TOKYO.green },
  { tag: [t.number, t.bool, t.null, t.atom], color: TOKYO.orange },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: TOKYO.blue },
  { tag: [t.propertyName, t.attributeName], color: TOKYO.green },
  { tag: [t.typeName, t.className, t.namespace], color: TOKYO.cyan },
  { tag: [t.definition(t.variableName), t.variableName, t.local(t.variableName)], color: TOKYO.fg },
  { tag: [t.tagName, t.angleBracket], color: TOKYO.red },
  { tag: [t.heading, t.strong], color: TOKYO.blue, fontWeight: "600" },
  { tag: [t.link, t.url], color: TOKYO.cyan, textDecoration: "underline" },
  { tag: t.invalid, color: TOKYO.red },
  { tag: [t.meta, t.documentMeta, t.annotation], color: TOKYO.fgGutter },
]);

const lang = (parser: StreamParser<unknown>): Extension => StreamLanguage.define(parser);

// Extension (lowercased, no dot) → CodeMirror stream parser.
const BY_EXT: Record<string, StreamParser<unknown>> = {
  js: javascript, mjs: javascript, cjs: javascript, jsx: javascript,
  ts: typescript, mts: typescript, cts: typescript, tsx: typescript,
  json: json, jsonc: json,
  py: python, pyi: python,
  rs: rust,
  go: go,
  rb: ruby, gemspec: ruby,
  swift: swift,
  c, h: c,
  cpp, cc: cpp, cxx: cpp, hpp: cpp, hh: cpp,
  java, kt: kotlin, kts: kotlin, scala, sc: scala,
  cs: csharp, m: objectiveC, mm: objectiveC, dart,
  css, scss: sCSS, less, sass: sCSS,
  html, htm: html, xml, svg: xml, xhtml: html, vue: html,
  sh: shell, bash: shell, zsh: shell, fish: shell,
  yaml, yml: yaml,
  toml,
  lua, pl: perl, pm: perl,
  hs: haskell,
  dockerfile: dockerFile,
  ini: properties, conf: properties, env: properties, properties,
  sql: standardSQL, ddl: standardSQL,
  diff, patch: diff,
};

// Filenames that carry no extension but map to a known mode.
const BY_NAME: Record<string, StreamParser<unknown>> = {
  dockerfile: dockerFile, ".env": properties, makefile: properties, ".gitconfig": properties,
};

const SYNTAX = syntaxHighlighting(tokyoHighlight);

/**
 * The highlighting extension for a given filename. Always returns the Tokyo
 * Night highlight style; when the extension is recognised it also installs the
 * matching language parser so tokens actually get coloured. Unknown types fall
 * back to highlight-style-only (no-op), keeping plain text readable.
 */
export function syntaxFor(fileName: string | null | undefined): Extension {
  if (!fileName) return SYNTAX;
  const name = fileName.toLowerCase();
  const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  const parser = BY_EXT[ext] ?? BY_NAME[name];
  return parser ? [lang(parser), SYNTAX] : SYNTAX;
}
