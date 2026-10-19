import { describe, expect, it } from "vitest";

import { DEFAULT_SHORTCUTS, findConflicts, type ShortcutDefinition } from "~/lib/shortcuts";

import { MODULE_SHORTCUTS } from "./modules";

const ALL: readonly ShortcutDefinition[] = [...DEFAULT_SHORTCUTS, ...MODULE_SHORTCUTS];
const byId = (id: string) => ALL.find((definition) => definition.id === id);
/** Every shortcut bound to `keys`, with its scope. */
const onKey = (keys: string) =>
  ALL.filter((definition) => definition.keys === keys).map((d) => `${d.id} (${d.scope})`);

describe("the app's shortcuts", () => {
  it("have no conflicts, defaults and modules together", () => {
    expect(findConflicts(ALL)).toEqual([]);
  });

  it("put Snooze on S and Pin on P, for the selected item", () => {
    expect(byId("item.snooze")).toMatchObject({ keys: "s", scope: "item" });
    expect(byId("conversations.star")).toMatchObject({ keys: "p", scope: "item" });
    expect(byId("item.later")).toBeUndefined();
    expect(byId("item.paid")).toBeUndefined();
  });

  it("share S and P only with one-question pages, which never show an item", () => {
    // Scout's questions skip with S and sign-in offers a passkey with P: both
    // live in the `focus` scope, which is never live with `item`.
    expect(onKey("s").toSorted()).toEqual(["item.snooze (item)", "scout.skip (focus)"]);
    expect(onKey("p").toSorted()).toEqual(["conversations.star (item)", "sign_in.passkey (focus)"]);
    expect(
      findConflicts([byId("item.snooze"), byId("scout.skip")].flatMap((d) => d ?? [])),
    ).toEqual([]);
  });

  it("walk the selected row's actions with ← and →, and take its own actions on 1 to 3", () => {
    expect(byId("item.next_action")?.keys).toBe("arrowright");
    expect(byId("item.previous_action")?.keys).toBe("arrowleft");
    expect(["item.choice1", "item.choice2", "item.choice3"].map((id) => byId(id)?.keys)).toEqual([
      "1",
      "2",
      "3",
    ]);
  });

  it("put Delete on # and Report spam on !, and Sent, Archive, Spam and Trash on G 5 to 8", () => {
    expect(byId("conversations.trash")).toMatchObject({ keys: "#", scope: "item" });
    expect(byId("conversations.report_spam")).toMatchObject({ keys: "!", scope: "item" });
    expect(onKey("#")).toEqual(["conversations.trash (item)"]);
    expect(onKey("!")).toEqual(["conversations.report_spam (item)"]);
    expect(
      ["inbox.view.sent", "inbox.view.archive", "inbox.view.spam", "inbox.view.trash"].map(
        (id) => byId(id)?.keys,
      ),
    ).toEqual(["g 5", "g 6", "g 7", "g 8"]);
  });
});
