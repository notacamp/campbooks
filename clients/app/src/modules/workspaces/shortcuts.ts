import type { ShortcutDefinition } from "~/lib/shortcuts";

/**
 * Shortcuts for `workspaces` operations, keyed by their OpenAPI operationId
 * (ADR 0013). The module manifest hands them to the app's registry.
 */
export const WORKSPACES_SHORTCUTS = [
  {
    id: "workspaces.create",
    keys: "n w",
    scope: "global",
    group: "general",
    label: "workspaces:shortcuts.create",
  },
  {
    id: "workspaces.invite_member",
    keys: "i",
    scope: "list",
    group: "general",
    label: "workspaces:shortcuts.invite",
  },
] as const satisfies readonly ShortcutDefinition[];
