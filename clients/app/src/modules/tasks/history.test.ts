import { describe, expect, it } from "vitest";

import en from "./i18n/en.json";
import type { TaskHistoryEntry } from "./api/queries";
import { historyLine, visibleHistory, type HistoryContext } from "./history";

const ME = "me";
const NAMES = new Map([
  [ME, "Ana Duarte"],
  ["sofia", "Sofia Pessoa"],
  ["rui", "Rui Matos"],
]);

/** `t` over the English strings, with `{{x}}` filled in. */
function t(key: string, options: Record<string, unknown> = {}): string {
  const found = key
    .split(".")
    .reduce<unknown>((node, part) => (node as Record<string, unknown> | undefined)?.[part], en);
  if (typeof found !== "string") {
    throw new TypeError(`no string for ${key}`);
  }
  return found.replaceAll(/\{\{(\w+)\}\}/gu, (_match, name: string) => String(options[name]));
}

const context = (over: Partial<HistoryContext> = {}): HistoryContext => ({
  t,
  locale: "en",
  me: ME,
  memberName: (id) => NAMES.get(id),
  sourceName: "Marta Ferreira",
  ...over,
});

const entry = (what: string, over: Partial<TaskHistoryEntry> = {}): TaskHistoryEntry =>
  ({
    id: "h",
    at: "2026-10-13T14:20:00Z",
    actor: "member",
    actor_member: ME,
    what,
    detail: {},
    ...over,
  }) as TaskHistoryEntry;

const line = (what: string, over: Partial<TaskHistoryEntry> = {}, c = context(), verb = "do") =>
  historyLine(entry(what, over), verb as "do", c);

describe("a task's history, in words", () => {
  it("says who made it, and when it is due", () => {
    expect(line("created")).toBe("You made it");
    expect(line("created", { actor_member: "sofia" })).toBe("Sofia Pessoa made it");
    expect(line("created", { detail: { due_on: "2026-10-16" } })).toBe("You made it, due Fri 16");
    expect(line("created", { actor: "scout", actor_member: null })).toBe(
      "Scout made it from Marta Ferreira’s email",
    );
    expect(
      line("created", { actor: "scout", actor_member: null, detail: { due_on: "2026-10-16" } }),
    ).toBe("Scout made it from Marta Ferreira’s email, due Fri 16");
    expect(
      line("created", { actor: "scout", actor_member: null }, context({ sourceName: undefined })),
    ).toBe("Scout made it");
  });

  it("lists the fields an edit changed", () => {
    expect(line("edited", { detail: { fields: ["title", "due"] } })).toBe(
      "You changed the title and the due date",
    );
    expect(line("edited", { detail: { fields: ["title", "task_verb", "due"] } })).toBe(
      "You changed the title, the kind, and the due date",
    );
    expect(line("edited")).toBe("You changed it");
  });

  it("says what happened to the date and the plan", () => {
    expect(line("due_changed", { detail: { due_on: "2026-10-20" } })).toBe(
      "You set the due date to Tue 20",
    );
    expect(line("due_changed")).toBe("You removed the due date");
    expect(line("planned", { detail: { plan: "date", plan_on: "2026-10-14" } })).toBe(
      "You planned it for Wed 14",
    );
    expect(
      line("planned", { detail: { plan: "later", later_until: "2026-11-02T09:00:00Z" } }),
    ).toMatch(/^You put it off until .*2/u);
    expect(line("planned", { detail: { plan: "none" } })).toBe("You took it off the plan");
    expect(line("planned")).toBe("You changed it");
  });

  it("says how it ended or came back", () => {
    expect(line("done")).toBe("You marked it done");
    expect(line("done", {}, context(), "pay")).toBe("You marked it paid");
    expect(line("not_needed", { actor_member: "rui" })).toBe("Rui Matos marked it not needed");
    expect(line("reopened")).toBe("You opened it again");
  });

  it("names who it was handed to and taken back from, and says you", () => {
    expect(line("handed_off", { detail: { member: "rui" } })).toBe("You handed it to Rui Matos");
    expect(line("handed_off", { actor_member: "sofia", detail: { member: ME } })).toBe(
      "Sofia Pessoa handed it to you",
    );
    expect(line("taken_back", { detail: { member: "rui" } })).toBe(
      "You took it back from Rui Matos",
    );
    expect(line("taken_back", { actor_member: "sofia", detail: { member: ME } })).toBe(
      "Sofia Pessoa took it back from you",
    );
  });

  it("says who a member is when the list does not have them: a teammate", () => {
    expect(line("created", { actor_member: "gone" })).toBe("A teammate made it");
    expect(line("handed_off", { detail: { member: "gone" } })).toBe("You handed it to A teammate");
    expect(line("done", { actor_member: null })).toBe("A teammate marked it done");
  });

  it("tells held time, the calendar moving it, and releasing it", () => {
    const held = { held_from: "2026-10-14T10:00:00Z", held_to: "2026-10-14T10:30:00Z" };
    expect(line("held", { detail: held })).toMatch(/^You held time Wed \d\d:\d\d–\d\d:\d\d$/u);
    expect(line("held", { actor: "calendar", actor_member: null, detail: held })).toMatch(
      /^The calendar moved the held time to Wed /u,
    );
    expect(line("released")).toBe("You released the held time");
  });

  it("tells the reply and its question", () => {
    expect(line("replied", { actor: "scout", actor_member: null })).toBe(
      "You replied in the thread. Scout asked if it was done.",
    );
    expect(line("reply_answered")).toBe("You said it isn’t done yet");
    expect(line("reply_answered", { actor_member: "sofia" })).toBe(
      "Sofia Pessoa said it isn’t done yet",
    );
    expect(line("reply_unanswered")).toBe("You undid “Not yet”");
  });

  it("does not crash on a kind it does not know", () => {
    expect(line("something_new", { actor_member: "rui" })).toBe("Rui Matos changed it");
    expect(line("something_new", { actor: "scout", actor_member: null })).toBe("Scout changed it");
  });
});

describe("folding a long history", () => {
  const ids = Array.from({ length: 12 }, (_, index) => index + 1);

  it("shows everything up to eight entries", () => {
    expect(visibleHistory(ids.slice(0, 8), false)).toEqual(ids.slice(0, 8));
  });

  it("shows the first and the latest six past eight, and all when expanded", () => {
    expect(visibleHistory(ids, false)).toEqual([1, 7, 8, 9, 10, 11, 12]);
    expect(visibleHistory(ids, true)).toEqual(ids);
  });
});
