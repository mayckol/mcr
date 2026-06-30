// Configurable keyboard shortcuts. Chords are normalized strings like
// "mod+z" / "mod+shift+z", where `mod` = Cmd on macOS, Ctrl elsewhere.

export type Action =
  | "undo"
  | "redo"
  | "applyAll"
  | "applyLeft"
  | "applyRight"
  | "next"
  | "prev";

export const ACTION_LABELS: Record<Action, string> = {
  undo: "Undo",
  redo: "Redo",
  applyAll: "Apply all non-conflicting",
  applyLeft: "Apply all from left",
  applyRight: "Apply all from right",
  next: "Next change",
  prev: "Previous change",
};

export const DEFAULT_BINDINGS: Record<Action, string> = {
  undo: "mod+z",
  redo: "mod+shift+z",
  applyAll: "mod+alt+a",
  applyLeft: "mod+alt+arrowleft",
  applyRight: "mod+alt+arrowright",
  next: "alt+arrowdown",
  prev: "alt+arrowup",
};

const STORAGE_KEY = "mcr.keymap";
const MODS = ["mod", "cmd", "command", "meta", "ctrl", "control", "alt", "option", "shift"];

const isMac =
  typeof navigator !== "undefined" && /mac|iphone|ipad/i.test(navigator.platform || navigator.userAgent);

function keyToken(e: KeyboardEvent): string | null {
  const c = e.code;
  if (c?.startsWith("Key")) return c.slice(3).toLowerCase();
  if (c?.startsWith("Digit")) return c.slice(5);
  const k = e.key;
  if (["Meta", "Control", "Shift", "Alt", "OS"].includes(k)) return null;
  return k.toLowerCase();
}

/** Build the normalized chord for a key event, or null for a lone modifier. */
export function chordFromEvent(e: KeyboardEvent): string | null {
  const k = keyToken(e);
  if (!k) return null;
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("mod");
  if (e.altKey) parts.push("alt");
  if (e.shiftKey) parts.push("shift");
  parts.push(k);
  return parts.join("+");
}

/** Canonicalize an arbitrary chord string to `mod+alt+shift+key` order. */
export function normalize(chord: string): string {
  const parts = chord.toLowerCase().split("+").map((s) => s.trim()).filter(Boolean);
  const mod = parts.some((p) => ["mod", "cmd", "command", "meta", "ctrl", "control"].includes(p));
  const alt = parts.some((p) => ["alt", "option"].includes(p));
  const shift = parts.includes("shift");
  const key = parts.filter((p) => !MODS.includes(p)).join("");
  const out: string[] = [];
  if (mod) out.push("mod");
  if (alt) out.push("alt");
  if (shift) out.push("shift");
  if (key) out.push(key);
  return out.join("+");
}

const SYMBOLS: Record<string, string> = {
  arrowleft: "←",
  arrowright: "→",
  arrowup: "↑",
  arrowdown: "↓",
  enter: "⏎",
  escape: "Esc",
  " ": "Space",
  space: "Space",
};

/** Human-readable chord, e.g. "mod+shift+z" -> "⌘⇧Z" (mac) / "Ctrl+Shift+Z". */
export function displayChord(chord: string): string {
  const parts = normalize(chord).split("+");
  const out: string[] = [];
  for (const p of parts) {
    if (p === "mod") out.push(isMac ? "⌘" : "Ctrl");
    else if (p === "alt") out.push(isMac ? "⌥" : "Alt");
    else if (p === "shift") out.push(isMac ? "⇧" : "Shift");
    else out.push(SYMBOLS[p] ?? p.toUpperCase());
  }
  return isMac ? out.join("") : out.join("+");
}

export class Shortcuts {
  private bindings: Record<Action, string>;
  private handlers: Partial<Record<Action, () => void>> = {};
  private capturing = false;

  constructor() {
    this.bindings = this.load();
  }

  private load(): Record<Action, string> {
    try {
      const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
      return { ...DEFAULT_BINDINGS, ...raw };
    } catch {
      return { ...DEFAULT_BINDINGS };
    }
  }

  private persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.bindings));
    } catch {
      /* storage unavailable — keep in-memory bindings */
    }
  }

  get(action: Action): string {
    return this.bindings[action];
  }

  all(): Record<Action, string> {
    return { ...this.bindings };
  }

  set(action: Action, chord: string) {
    this.bindings[action] = normalize(chord);
    this.persist();
  }

  reset() {
    this.bindings = { ...DEFAULT_BINDINGS };
    this.persist();
  }

  on(handlers: Partial<Record<Action, () => void>>) {
    this.handlers = handlers;
  }

  /** Suspend dispatch while the rebind UI is capturing a key. */
  suspend(on: boolean) {
    this.capturing = on;
  }

  attach(target: Window | HTMLElement = window) {
    target.addEventListener("keydown", (e) => this.handle(e as KeyboardEvent));
  }

  private handle(e: KeyboardEvent) {
    if (this.capturing) return;
    const chord = chordFromEvent(e);
    if (!chord) return;
    for (const action of Object.keys(this.bindings) as Action[]) {
      if (this.bindings[action] === chord && this.handlers[action]) {
        e.preventDefault();
        this.handlers[action]!();
        return;
      }
    }
  }
}
