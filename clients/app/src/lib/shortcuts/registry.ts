import { normalizeBinding, parseBinding } from "./keys";

/**
 * Where a shortcut is live. `global` works anywhere; `list` and `item` while a
 * list or a selected item binds a handler (often both at once); `compose`
 * while writing. Two shortcuts conflict when they share keys, or one's keys
 * start the other's sequence, in scopes that can be live together: `global`
 * overlaps every scope, `list` and `item` overlap each other.
 */
export type ShortcutScope = "global" | "list" | "item" | "compose";

/** Which heading the shortcut sheet and command menu list it under. */
export type ShortcutGroup = "navigation" | "general" | "list" | "item" | "compose";

export interface ShortcutDefinition {
  /**
   * The action this shortcut runs. Declared actions (ADR 0008) register with
   * their action name, so one id names the action everywhere: the shortcut,
   * the command menu, and later the API, MCP and assistant tools.
   */
  id: string;
  /** Default binding (see keys.ts for the grammar). */
  keys: string;
  scope: ShortcutScope;
  group: ShortcutGroup;
  /** i18n key with its namespace, e.g. "shortcuts:nav.inbox". */
  label: string;
  /** Fire even while the user types in a field (Escape, ⌘K, ⌘↵). */
  allowInInputs?: boolean;
}

export type ShortcutHandler = (event: KeyboardEvent | null) => void;

export interface ShortcutConflict {
  a: string;
  b: string;
  reason: "same-keys" | "prefix";
}

const SCREEN_SCOPES = new Set<ShortcutScope>(["list", "item"]);

function scopesOverlap(a: ShortcutScope, b: ShortcutScope): boolean {
  return (
    a === b || a === "global" || b === "global" || (SCREEN_SCOPES.has(a) && SCREEN_SCOPES.has(b))
  );
}

/** Every pair of definitions that would fight over the same keys. */
export function findConflicts(definitions: readonly ShortcutDefinition[]): ShortcutConflict[] {
  const conflicts: ShortcutConflict[] = [];
  const normalized = definitions.map((definition) => ({
    definition,
    chords: normalizeBinding(definition.keys).split(" "),
  }));
  for (const [index, first] of normalized.entries()) {
    for (const second of normalized.slice(index + 1)) {
      if (!scopesOverlap(first.definition.scope, second.definition.scope)) {
        continue;
      }
      const [shorter, longer] =
        first.chords.length <= second.chords.length ? [first, second] : [second, first];
      const isPrefix = shorter.chords.every((chord, position) => longer.chords[position] === chord);
      if (isPrefix) {
        conflicts.push({
          a: first.definition.id,
          b: second.definition.id,
          reason: shorter.chords.length === longer.chords.length ? "same-keys" : "prefix",
        });
      }
    }
  }
  return conflicts;
}

/**
 * The one shortcut registry: definitions (what exists, its default keys and
 * label) plus the handlers screens bind at runtime. The shortcut sheet, the
 * command menu and every keyboard hint read from here, so a hint can never
 * drift from its binding.
 */
export class ShortcutRegistry {
  readonly #definitions = new Map<string, ShortcutDefinition>();
  readonly #handlers = new Map<string, ShortcutHandler[]>();
  readonly #listeners = new Set<() => void>();
  #version = 0;

  constructor(definitions: readonly ShortcutDefinition[] = []) {
    this.define(definitions);
  }

  /** Adds definitions. Throws on a duplicate id, an invalid binding or a conflict. */
  define(definitions: readonly ShortcutDefinition[]): void {
    for (const definition of definitions) {
      if (this.#definitions.has(definition.id)) {
        throw new Error(`Shortcut "${definition.id}" is already defined`);
      }
      parseBinding(definition.keys);
    }
    const conflicts = findConflicts([...this.#definitions.values(), ...definitions]);
    const [conflict] = conflicts;
    if (conflict !== undefined) {
      throw new Error(
        `Shortcut conflict (${conflict.reason}): "${conflict.a}" and "${conflict.b}"`,
      );
    }
    for (const definition of definitions) {
      this.#definitions.set(definition.id, definition);
    }
    this.#changed();
  }

  definitions(): ShortcutDefinition[] {
    return [...this.#definitions.values()];
  }

  get(id: string): ShortcutDefinition | undefined {
    return this.#definitions.get(id);
  }

  /** Binds a handler; the most recent binding wins. Returns the unbind function. */
  bind(id: string, handler: ShortcutHandler): () => void {
    if (!this.#definitions.has(id)) {
      throw new Error(`Unknown shortcut "${id}": define it before binding a handler`);
    }
    const stack = this.#handlers.get(id) ?? [];
    stack.push(handler);
    this.#handlers.set(id, stack);
    this.#changed();
    return () => {
      const current = this.#handlers.get(id) ?? [];
      const position = current.lastIndexOf(handler);
      if (position !== -1) {
        current.splice(position, 1);
        this.#changed();
      }
    };
  }

  isBound(id: string): boolean {
    return (this.#handlers.get(id)?.length ?? 0) > 0;
  }

  /** Runs the active handler for `id`. Returns false when nothing is bound. */
  run(id: string, event: KeyboardEvent | null = null): boolean {
    const handler = this.#handlers.get(id)?.at(-1);
    if (handler === undefined) {
      return false;
    }
    handler(event);
    return true;
  }

  /** For useSyncExternalStore: changes whenever definitions or bindings change. */
  subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  };

  getVersion = (): number => this.#version;

  #changed(): void {
    this.#version += 1;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}
