/**
 * Key bindings as strings, one grammar everywhere:
 *
 *   "e"            a single key
 *   "?"            a symbol (Shift is implied by the character)
 *   "mod+k"        ⌘K on Apple platforms, Ctrl+K elsewhere
 *   "mod+shift+w"  modifiers joined with "+"
 *   "g i"          a sequence: G, then I
 *   "enter", "escape", "arrowup", ...  named keys (KeyboardEvent.key, lowercased)
 */

export type Platform = "apple" | "other";

export interface Chord {
  /** KeyboardEvent.key, lowercased ("k", "?", "enter"). */
  key: string;
  mod: boolean;
  shift: boolean;
  alt: boolean;
}

const MODIFIERS = new Set(["mod", "shift", "alt"]);

/** Parses one chord ("mod+shift+w"). Throws on an empty or unknown modifier. */
function parseChord(text: string): Chord {
  const parts = text.toLowerCase().split("+");
  const key = parts.pop() ?? "";
  if (key === "" || MODIFIERS.has(key)) {
    throw new Error(`Invalid key binding "${text}": it needs a key after the modifiers`);
  }
  for (const part of parts) {
    if (!MODIFIERS.has(part)) {
      throw new Error(`Invalid key binding "${text}": unknown modifier "${part}"`);
    }
  }
  return {
    key,
    mod: parts.includes("mod"),
    shift: parts.includes("shift"),
    alt: parts.includes("alt"),
  };
}

/** Parses a binding into its chords: "g i" → [G, I]. */
export function parseBinding(binding: string): Chord[] {
  const chords = binding.trim().split(/\s+/).map(parseChord);
  if (chords.length === 0) {
    throw new Error("Empty key binding");
  }
  return chords;
}

function isPrintableSymbol(key: string): boolean {
  return key.length === 1 && key.toLowerCase() === key.toUpperCase();
}

const chordId = (chord: Chord): string =>
  [chord.mod && "mod", chord.alt && "alt", chord.shift && "shift", chord.key]
    .filter(Boolean)
    .join("+");

/** A canonical string for a binding, so equal bindings compare equal. */
export function normalizeBinding(binding: string): string {
  return parseBinding(binding).map(chordId).join(" ");
}

/** The chord a keyboard event represents, or null for a lone modifier key. */
export function chordFromEvent(event: KeyboardEvent, platform: Platform): Chord | null {
  let key = event.key.toLowerCase();
  let byDigit = false;
  // ⌘⇧8 types "*" on a US keyboard: with a modifier held, a digit key is
  // the digit it is labelled with, whatever Shift types.
  const digit = /^Digit(\d)$/u.exec(event.code);
  if (digit?.[1] !== undefined && event.shiftKey && (event.metaKey || event.ctrlKey)) {
    key = digit[1];
    byDigit = true;
  }
  if (
    ["shift", "meta", "control", "alt", "altgraph", "capslock", "dead", "unidentified"].includes(
      key,
    )
  ) {
    return null;
  }
  return {
    key,
    mod: platform === "apple" ? event.metaKey : event.ctrlKey,
    // A symbol like "?" already encodes Shift in the character itself.
    shift: byDigit || !isPrintableSymbol(key) ? event.shiftKey : false,
    alt: event.altKey,
  };
}

export const chordsEqual = (a: Chord, b: Chord): boolean => chordId(a) === chordId(b);

/** Best-effort platform detection for labels and the `mod` key. */
export function detectPlatform(
  nav: Pick<Navigator, "platform" | "userAgent"> = navigator,
): Platform {
  return /mac|iphone|ipad|ipod/i.test(`${nav.platform} ${nav.userAgent}`) ? "apple" : "other";
}

const NAMED: Record<string, string> = {
  enter: "↵",
  escape: "Esc",
  arrowup: "↑",
  arrowdown: "↓",
  arrowleft: "←",
  arrowright: "→",
  backspace: "⌫",
  tab: "Tab",
  " ": "Space",
};

const ARIA_NAMED: Record<string, string> = {
  enter: "Enter",
  escape: "Escape",
  arrowup: "ArrowUp",
  arrowdown: "ArrowDown",
  arrowleft: "ArrowLeft",
  arrowright: "ArrowRight",
  backspace: "Backspace",
  tab: "Tab",
  " ": "Space",
};

const MODIFIER_LABELS: Record<Platform, { mod: string; shift: string; alt: string }> = {
  apple: { mod: "⌘", shift: "⇧", alt: "⌥" },
  other: { mod: "Ctrl", shift: "Shift", alt: "Alt" },
};

/** Keycaps to draw for a binding, one array per chord: "mod+k" → [["⌘", "K"]]. */
export function formatBinding(binding: string, platform: Platform): string[][] {
  const labels = MODIFIER_LABELS[platform];
  return parseBinding(binding).map((chord) => [
    ...(chord.mod ? [labels.mod] : []),
    ...(chord.alt ? [labels.alt] : []),
    ...(chord.shift ? [labels.shift] : []),
    NAMED[chord.key] ?? chord.key.toUpperCase(),
  ]);
}

/**
 * The binding in `aria-keyshortcuts` syntax ("Meta+K"), or undefined for
 * sequences, which that attribute cannot express.
 */
export function ariaKeyShortcut(binding: string, platform: Platform): string | undefined {
  const chords = parseBinding(binding);
  const [chord] = chords;
  if (chord === undefined || chords.length > 1) {
    return undefined;
  }
  const key = ARIA_NAMED[chord.key] ?? chord.key.toUpperCase();
  return [
    chord.mod && (platform === "apple" ? "Meta" : "Control"),
    chord.alt && "Alt",
    chord.shift && "Shift",
    key,
  ]
    .filter(Boolean)
    .join("+");
}
