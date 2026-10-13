import { chordFromEvent, chordsEqual, parseBinding, type Chord, type Platform } from "./keys";
import type { ShortcutRegistry } from "./registry";

/** How long the second key of a sequence ("g", then "i") may take. */
export const SEQUENCE_TIMEOUT_MS = 1500;

const TEXT_INPUT_TYPES = new Set([
  "text",
  "search",
  "email",
  "url",
  "tel",
  "password",
  "number",
  "date",
  "datetime-local",
  "month",
  "time",
  "week",
]);

/** True when keys typed at `target` are text, not commands. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUT_TYPES.has(target.type);
  }
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return true;
  }
  return target.closest('[contenteditable]:not([contenteditable="false"])') !== null;
}

export interface DispatcherOptions {
  timeoutMs?: number;
  now?: () => number;
}

/**
 * Builds the one keydown listener: matches chords and sequences against the
 * shortcuts that have a bound handler, ignores keys typed into fields (unless
 * the shortcut allows it) and runs the matching handler.
 */
export function createDispatcher(
  registry: ShortcutRegistry,
  platform: Platform,
  { timeoutMs = SEQUENCE_TIMEOUT_MS, now = Date.now }: DispatcherOptions = {},
): (event: KeyboardEvent) => void {
  const parsed = new Map<string, Chord[]>();
  const chordsOf = (keys: string): Chord[] => {
    let chords = parsed.get(keys);
    if (chords === undefined) {
      chords = parseBinding(keys);
      parsed.set(keys, chords);
    }
    return chords;
  };
  let buffer: Chord[] = [];
  let lastAt = 0;

  const attempt = (candidate: Chord[], event: KeyboardEvent, typing: boolean): boolean => {
    const live = registry
      .definitions()
      .filter((definition) => registry.isBound(definition.id))
      .filter((definition) => !typing || definition.allowInInputs === true);
    const startsWith = (chords: Chord[]) =>
      candidate.every((chord, index) => {
        const other = chords[index];
        return other !== undefined && chordsEqual(chord, other);
      });

    const match = live.find((definition) => {
      const chords = chordsOf(definition.keys);
      return chords.length === candidate.length && startsWith(chords);
    });
    if (match !== undefined) {
      buffer = [];
      event.preventDefault();
      registry.run(match.id, event);
      return true;
    }
    if (
      live.some(
        (definition) =>
          chordsOf(definition.keys).length > candidate.length &&
          startsWith(chordsOf(definition.keys)),
      )
    ) {
      buffer = candidate;
      lastAt = now();
      return true;
    }
    return false;
  };

  return (event: KeyboardEvent) => {
    if (event.defaultPrevented || event.isComposing) {
      return;
    }
    const chord = chordFromEvent(event, platform);
    if (chord === null) {
      return;
    }
    const typing = isTypingTarget(event.target);
    if (now() - lastAt > timeoutMs) {
      buffer = [];
    }
    if (attempt([...buffer, chord], event, typing)) {
      return;
    }
    // The pending sequence did not continue: try this key on its own.
    const hadBuffer = buffer.length > 0;
    buffer = [];
    if (hadBuffer) {
      attempt([chord], event, typing);
    }
  };
}
