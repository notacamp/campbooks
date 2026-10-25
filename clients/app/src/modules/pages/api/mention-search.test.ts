import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, IDEAS } from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi } from "../../../../test/render";

import { searchMentions } from "./mention-search";

const signal = new AbortController().signal;

const hit = (id: string, title: string, type = "conversations.conversation") => ({
  record: { type, id },
  container: { type: "conversations.inbox", id: "i1" },
  facets: {},
  occurred_at: "2026-10-12T09:00:00Z",
  snippet: [],
  title: [{ text: title, matched: false }],
  from: { name: "Marta Ferreira", address: "marta@example.com" },
});

function api() {
  return mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/correspondents`]: {
      body: {
        correspondents: [
          { address: "Marta@Example.com", name: "Marta Ferreira", wrote: 1, received: 2 },
        ],
      },
    },
    [`GET ${W}/search`]: {
      body: {
        hits: [hit("t1", "Lease for Marta"), hit("x1", "Not a thread", "pages.page")],
        indexing: false,
        query: { text: "", filters: [], warnings: [] },
      },
    },
    [`GET ${W}/tasks`]: {
      body: {
        tasks: [
          { id: "k1", title: "Send the lease" },
          { id: "k2", title: "Pay the bill" },
        ],
      },
    },
    [`GET ${W}/schedule/events`]: {
      body: { events: [{ id: "e1", title: "Kickoff lunch" }] },
    },
  });
}

describe("searching for what @ can mention", () => {
  it("groups what it finds by kind, in the order the menu shows them", async () => {
    api();
    const groups = await searchMentions(DUARTE.id, "le", [HANDBOOK, IDEAS], signal);
    expect(groups.map((group) => group.kind)).toEqual(["people", "threads", "tasks", "events"]);
  });

  it("finds people by name among members and correspondents, never by a typed address", async () => {
    api();
    const groups = await searchMentions(DUARTE.id, "marta", [], signal);
    const people = groups.find((group) => group.kind === "people")?.candidates ?? [];
    expect(people).toEqual([
      {
        type: "person",
        id: "marta@example.com",
        label: "Marta Ferreira",
        detail: "Marta@Example.com",
      },
    ]);
  });

  it("filters members, tasks and pages by name, ignoring case and accents", async () => {
    api();
    const groups = await searchMentions(DUARTE.id, "RUI", [IDEAS, HANDBOOK], signal);
    expect(
      groups.find((group) => group.kind === "people")?.candidates.map((c) => c.label),
    ).toContain("Rui Matos");
    const tasks = await searchMentions(DUARTE.id, "lease", [], signal);
    expect(tasks.find((group) => group.kind === "tasks")?.candidates).toEqual([
      { type: "tasks.task", id: "k1", label: "Send the lease" },
    ]);
    const pages = await searchMentions(DUARTE.id, "hand", [IDEAS, HANDBOOK], signal);
    expect(pages.find((group) => group.kind === "pages")?.candidates.map((c) => c.id)).toEqual([
      HANDBOOK.id,
    ]);
  });

  it("offers only the pages the viewer's tree holds, and nothing of another kind as a thread", async () => {
    api();
    const groups = await searchMentions(DUARTE.id, "ma", [], signal);
    expect(groups.find((group) => group.kind === "pages")).toBeUndefined();
    const threads = groups.find((group) => group.kind === "threads")?.candidates ?? [];
    expect(threads.map((thread) => thread.id)).toEqual(["t1"]);
  });

  it("answers what it can when a search fails, and does not look for threads on one letter", async () => {
    const mocked = mockApi({ [`GET ${W}/members`]: { body: { members: TEAM } } });
    const groups = await searchMentions(DUARTE.id, "r", [], signal);
    expect(groups.map((group) => group.kind)).toEqual(["people"]);
    expect(mocked.calls.map((call) => call.route)).not.toContain(`GET ${W}/search`);
  });
});
