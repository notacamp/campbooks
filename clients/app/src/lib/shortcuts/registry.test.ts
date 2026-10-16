import { describe, expect, it, vi } from "vitest";

import { DEFAULT_SHORTCUTS } from "./catalog";
import { findConflicts, ShortcutRegistry, type ShortcutDefinition } from "./registry";

const define = (
  id: string,
  keys: string,
  scope: ShortcutDefinition["scope"] = "global",
): ShortcutDefinition => ({
  id,
  keys,
  scope,
  group: "general",
  label: `shortcuts:${id}`,
});

describe("the default shortcuts", () => {
  it("have no conflicts", () => {
    expect(findConflicts(DEFAULT_SHORTCUTS)).toEqual([]);
  });

  it("are client-only commands, outside the operationId namespace (ADR 0013)", () => {
    // Module names from docs/domain-map.md: an id starting with one of these
    // is reserved for that module's OpenAPI operationIds.
    const modules = new Set([
      "system",
      "workspaces",
      "connectors",
      "conversations",
      "parties",
      "attention",
      "search",
      "notifications",
      "schedule",
      "tasks",
      "automation",
      "documents",
      "money",
      "assistant",
      "records",
      "views",
    ]);
    const clashing = DEFAULT_SHORTCUTS.filter((definition) =>
      modules.has(definition.id.split(".", 1)[0] ?? ""),
    );

    expect(clashing).toEqual([]);
  });

  it("accept a server operation keyed by its operationId", () => {
    const registry = new ShortcutRegistry(DEFAULT_SHORTCUTS);

    expect(() => {
      registry.define([define("conversations.archive", "a", "item")]);
    }).not.toThrow();
    expect(registry.get("conversations.archive")?.keys).toBe("a");
  });

  it("have unique ids and namespaced labels", () => {
    const ids = DEFAULT_SHORTCUTS.map((definition) => definition.id);

    expect(new Set(ids).size).toBe(ids.length);
    expect(DEFAULT_SHORTCUTS.every((definition) => definition.label.startsWith("shortcuts:"))).toBe(
      true,
    );
  });
});

describe("findConflicts", () => {
  it("flags the same keys in overlapping scopes", () => {
    expect(findConflicts([define("a", "e", "item"), define("b", "e", "list")])).toEqual([
      { a: "a", b: "b", reason: "same-keys" },
    ]);
    expect(findConflicts([define("a", "mod+k"), define("b", "Mod+K", "compose")])).toHaveLength(1);
  });

  it("flags a key that starts another shortcut's sequence", () => {
    expect(findConflicts([define("go", "g"), define("inbox", "g i")])).toEqual([
      { a: "go", b: "inbox", reason: "prefix" },
    ]);
  });

  it("allows the same keys in scopes that are never live together", () => {
    expect(findConflicts([define("a", "mod+j", "compose"), define("b", "mod+j", "list")])).toEqual(
      [],
    );
    expect(findConflicts([define("a", "h", "item"), define("b", "g h")])).toEqual([]);
  });

  it("keeps a focus page's keys apart from lists and items, but not from global ones", () => {
    expect(findConflicts([define("a", "1", "focus"), define("b", "1", "item")])).toEqual([]);
    expect(findConflicts([define("a", "1", "focus"), define("b", "1")])).toHaveLength(1);
  });

  it("keeps a settings page's keys apart from the reader's, but not from global or list ones", () => {
    expect(findConflicts([define("a", "r", "settings"), define("b", "r", "item")])).toEqual([]);
    expect(findConflicts([define("a", "j", "settings"), define("b", "j", "list")])).toHaveLength(1);
    expect(findConflicts([define("a", "r", "settings"), define("b", "r", "focus")])).toEqual([]);
    expect(findConflicts([define("a", "c", "settings"), define("b", "c")])).toHaveLength(1);
  });
});

describe("ShortcutRegistry", () => {
  it("refuses duplicate ids, invalid bindings and conflicts", () => {
    const registry = new ShortcutRegistry([define("a", "e")]);

    expect(() => {
      registry.define([define("a", "x")]);
    }).toThrow(/already defined/);
    expect(() => {
      registry.define([define("b", "hyper+x")]);
    }).toThrow(/unknown modifier/);
    expect(() => {
      registry.define([define("c", "e")]);
    }).toThrow(/conflict \(same-keys\): "a" and "c"/);
  });

  it("runs the most recent handler and falls back when it unbinds", () => {
    const registry = new ShortcutRegistry([define("a", "e")]);
    const first = vi.fn();
    const second = vi.fn();
    registry.bind("a", first);
    const unbind = registry.bind("a", second);

    expect(registry.run("a")).toBe(true);
    expect(second).toHaveBeenCalledOnce();

    unbind();
    unbind();
    registry.run("a");

    expect(first).toHaveBeenCalledOnce();
  });

  it("reports nothing bound and refuses handlers for unknown ids", () => {
    const registry = new ShortcutRegistry([define("a", "e")]);

    expect(registry.isBound("a")).toBe(false);
    expect(registry.run("a")).toBe(false);
    expect(() => registry.bind("missing", vi.fn())).toThrow(/Unknown shortcut "missing"/);
  });

  it("notifies subscribers when definitions or bindings change", () => {
    const registry = new ShortcutRegistry();
    const listener = vi.fn();
    const unsubscribe = registry.subscribe(listener);
    const before = registry.getVersion();

    registry.define([define("a", "e")]);
    registry.bind("a", vi.fn());
    unsubscribe();
    registry.define([define("b", "x")]);

    expect(listener).toHaveBeenCalledTimes(2);
    expect(registry.getVersion()).toBe(before + 3);
    expect(registry.get("b")?.keys).toBe("x");
    expect(registry.definitions()).toHaveLength(2);
  });
});

describe("alternate keys", () => {
  const next: ShortcutDefinition = {
    ...define("list.next", "j", "list"),
    alternateKeys: ["arrowdown"],
  };

  it("are checked for conflicts like the main keys", () => {
    expect(findConflicts([next, define("scroll", "arrowdown", "item")])).toEqual([
      { a: "list.next", b: "scroll", reason: "same-keys" },
    ]);
  });

  it("are live only for a handler that takes them", () => {
    const registry = new ShortcutRegistry([next]);
    expect(registry.liveBindings("list.next")).toEqual([]);

    const unbind = registry.bind("list.next", vi.fn());
    expect(registry.liveBindings("list.next")).toEqual(["j", "arrowdown"]);

    registry.bind("list.next", vi.fn(), { alternates: false });
    expect(registry.liveBindings("list.next")).toEqual(["j"]);
    unbind();
  });

  it("must parse", () => {
    expect(() => new ShortcutRegistry([{ ...next, alternateKeys: ["shift+"] }])).toThrow(
      /Invalid key binding/,
    );
  });
});
