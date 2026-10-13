import type { ShortcutDefinition } from "~/lib/shortcuts";

import { CHOICE_SHORTCUTS } from "./model";

/**
 * `identity` operations' keys (by operationId, ADR 0013): adding a way in on
 * Settings → Ways to sign in. And the sign-in screen's keys: the ways in by position, and passkeys. They live
 * in the `focus` scope (one-question pages), so `1` and `2` don't fight the
 * list and item shortcuts, and the `?` sheet lists them only on that page.
 */
export const IDENTITY_SHORTCUTS = [
  {
    id: CHOICE_SHORTCUTS[0],
    keys: "1",
    scope: "focus",
    group: "general",
    label: "identity:shortcuts.choice1",
  },
  {
    id: CHOICE_SHORTCUTS[1],
    keys: "2",
    scope: "focus",
    group: "general",
    label: "identity:shortcuts.choice2",
  },
  {
    id: CHOICE_SHORTCUTS[2],
    keys: "3",
    scope: "focus",
    group: "general",
    label: "identity:shortcuts.choice3",
  },
  {
    id: "sign_in.passkey",
    keys: "p",
    scope: "focus",
    group: "general",
    label: "identity:shortcuts.passkey",
  },
  {
    id: "identity.start_link",
    keys: "a",
    scope: "list",
    group: "general",
    label: "identity:shortcuts.addWayIn",
  },
] as const satisfies readonly ShortcutDefinition[];
