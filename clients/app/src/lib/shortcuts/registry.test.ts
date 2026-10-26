import { UI_SHORTCUTS } from "@notacamp/ui/shortcuts";
import { describe, expect, it } from "vitest";

import { DEFAULT_SHORTCUTS } from "./catalog";
import { findConflicts, ShortcutRegistry, type ShortcutDefinition } from "./index";

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

  it("include the design system's own shortcuts, unchanged", () => {
    for (const definition of UI_SHORTCUTS) {
      expect(DEFAULT_SHORTCUTS).toContainEqual(definition);
    }
  });
});
