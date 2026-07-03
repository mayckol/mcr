import { StreamLanguage } from "@codemirror/language";
import type { StreamParser } from "@codemirror/language";
import type { Extension } from "@codemirror/state";

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

/**
 * The language parser extension for a given filename, so tokens get typed and
 * the theme's highlight style (installed separately, see theme/editor.ts) can
 * color them. Unknown types return an empty extension — plain text.
 */
export function syntaxFor(fileName: string | null | undefined): Extension {
  if (!fileName) return [];
  const name = fileName.toLowerCase();
  const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1) : "";
  const parser = BY_EXT[ext] ?? BY_NAME[name];
  return parser ? lang(parser) : [];
}
