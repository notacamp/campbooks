import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ScoutChatProvider, type OpenScoutChat } from "~/lib/scout-chat";

import {
  ACME,
  CLEARED,
  COUNTS,
  DIGEST,
  DIGEST_ITEM,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
  JONAS,
  LUZ,
  PEDRO,
  PHISH,
  PHISH_DETAIL,
  SOFIA,
} from "../../../../../test/demo/attention";
import { NOW, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders, replyStream } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

function renderHome(scout?: OpenScoutChat) {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () =>
      scout === undefined ? (
        <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />
      ) : (
        <ScoutChatProvider open={scout}>
          <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />
        </ScoutChatProvider>
      ),
  });
  const conversation = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream/$conversationId",
    validateSearch: (search: Record<string, unknown>) =>
      search["reply"] === true ? { reply: true } : {},
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, conversation]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

function api(
  items = ITEMS,
  live?: { items: typeof ITEMS },
  more: Parameters<typeof mockApi>[0] = {},
) {
  return mockApi({
    ...more,
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: live?.items ?? items, counts: COUNTS } },
    [`GET ${W}/conversations`]: {
      body: { conversations: [...NEEDS, SOFIA, DIGEST], next_cursor: null },
    },
    [`POST ${W}/conversations/${DIGEST.id}/need/resolve`]: { body: { ...DIGEST, need: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: { body: { ...PHISH, need: null } },
    [`POST ${W}/conversations/${PHISH.id}/need/reopen`]: { body: PHISH },
    [`POST ${W}/conversations/${MARTA.id}/need/resolve`]: { body: { ...MARTA, need: null } },
    [`POST ${W}/conversations/${JONAS.id}/need/resolve`]: { body: { ...JONAS, need: null } },
    [`POST ${W}/conversations/${PEDRO.id}/need/resolve`]: { body: { ...PEDRO, need: null } },
    [`POST ${W}/attention/${ITEMS[3]?.id ?? ""}/snooze`]: { body: ITEMS[3] },
    [`POST ${W}/conversations/${MARTA.id}/reply/suggestions`]: {
      body: {
        message_id: MARTA_DETAIL.messages.at(-1)?.id,
        intents: ["Say Thursday works", "Ask to move it"],
      },
    },
    [`POST ${W}/conversations/${MARTA.id}/reply/draft/stream`]: replyStream(
      MARTA_DETAIL.messages.at(-1)?.id ?? "",
      MARTA_REPLY,
    ),
    [`POST ${W}/drafts`]: { status: 201, body: MARTA_DRAFT },
    [`POST ${W}/drafts/${MARTA_DRAFT.id}/send`]: {
      status: 202,
      body: { ...MARTA_DRAFT, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
    },
  });
}

const MARTA_REPLY = "Hi Marta,\n\nThursday works.\n\nSam Rivera";
const MARTA_DRAFT = {
  ...REPLY,
  conversation_id: MARTA.id,
  message_id: MARTA_DETAIL.messages.at(-1)?.id ?? null,
  body: MARTA_REPLY,
};

/** The row (button) holding `text`, and whether it is the selected one. */
function isSelected(text: string): boolean {
  return screen.getByText(text).closest("button")?.hasAttribute("data-selected") === true;
}

describe("What needs you", () => {
  it("groups what needs you by band, Now first, in plain words", async () => {
    api();
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 4)).toEqual(["Now 2", "Today 1", "This week 2", "Whenever 1"]);
    expect(screen.getByText("Marta Ferreira is waiting for your reply")).toBeVisible();
    expect(screen.getByText("Due tomorrow")).toBeVisible();
    // The warning names the sender by address; no percentage anywhere.
    expect(screen.getByText("it-support@at3lier-sol.example")).toBeVisible();
    expect(document.body).not.toHaveTextContent("%");
    expect(screen.getByText("Replied · Tile samples")).toBeVisible();
  });

  it("says first what each row is and why it is there, and only its age at the end", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    const row = (text: string) => screen.getByText(text).closest("button");
    expect(row("This may be phishing. Don’t use its links.")?.textContent).toMatch(
      /Review · Possible scam · check first it-support@at3lier-sol.example/u,
    );
    expect(row("Pay Luz Energia")?.textContent).toMatch(/Pay · Overdue · 1 day Luz Energia/u);
    expect(screen.getByText("Overdue · 1 day")).toHaveAttribute("data-danger", "true");
    expect(row("Pedro Alves asked you to do something")?.textContent).toMatch(
      /Do · Due Fri Pedro Alves/u,
    );
    expect(row("Jonas Weber asked you a question")?.textContent).toMatch(
      /Reply · Jonas is waiting for your answer Jonas Weber/u,
    );
    // The due date is in the eyebrow, not at the row's end.
    expect(row("Marta Ferreira is waiting for your reply")?.textContent).not.toMatch(
      /Due tomorrow$/u,
    );
  });

  it("says why an opened item is here", async () => {
    api();
    renderHome();
    fireEvent.click(await screen.findByText("Marta Ferreira is waiting for your reply"));
    const why = await screen.findByRole("region", { name: "Why it’s here" });
    expect(why).toHaveTextContent("In Today because due tomorrow.");
    expect(why).toHaveTextContent(
      "Due today or tomorrow, planned for today, nudges that are due, and people who have waited 3 days or more.",
    );
  });

  it("ends the list with how many more tasks are in Tasks", async () => {
    mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [], tasks_elsewhere: 0 } }
          : { body: { items: ITEMS, counts: COUNTS, tasks_elsewhere: 4 } },
      [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
    });
    renderHome();
    expect(await screen.findByText("4 more in Tasks")).toBeVisible();
    const open = screen.getByRole("link", { name: /^Open Tasks/u });
    expect(open).toHaveAttribute("href", `/w/${DUARTE.id}/tasks`);
  });

  it("offers Continue with Scout after the need's own actions, on .", async () => {
    api();
    const scout = vi.fn();
    renderHome(scout);
    fireEvent.click(await screen.findByText("Marta Ferreira is waiting for your reply"));
    const group = await screen.findByRole("group", { name: "Actions for Marta Ferreira" });
    const actions = [...group.querySelectorAll("button")].map((b) => b.dataset["action"]);
    const at = actions.indexOf("scout");

    expect(at).toBeGreaterThan(0);
    expect(actions.slice(at + 1)).toEqual(["done", "snooze", "not_needed"]);
    const button = within(group).getByRole("button", { name: /Continue with Scout/u });
    expect(button).toHaveAttribute("aria-keyshortcuts", ".");
    expect(button).toHaveAttribute("data-ai", "true");

    fireEvent.keyDown(document, { key: "." });
    expect(scout).toHaveBeenCalledWith(
      { type: "conversations.conversation", id: MARTA.id },
      { from: button },
    );
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    fireEvent.click(button);
    expect(scout).toHaveBeenCalledTimes(2);
  });

  it("offers nothing of Scout where no chat is mounted", async () => {
    api();
    renderHome();
    fireEvent.click(await screen.findByText("Marta Ferreira is waiting for your reply"));
    await screen.findByRole("group", { name: "Actions for Marta Ferreira" });

    expect(screen.queryByRole("button", { name: /Continue with Scout/u })).not.toBeInTheDocument();
  });

  it("keeps the sender's words as plain text", async () => {
    api();
    renderHome();
    const quote = await screen.findByText(
      "Could you confirm Thursday’s walkthrough? The builders need to know by Wednesday.",
    );
    expect(quote.tagName).toBe("Q");
    expect(screen.queryAllByRole("link")).toHaveLength(0);
  });

  it("marks the warning safe with X and takes it back with Z", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "x" });
    await waitFor(() => {
      expect(screen.queryByText("This may be phishing. Don’t use its links.")).toBeNull();
    });
    // It names the need it acted on: the server refuses one that changed.
    expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toEqual({
      outcome: "not_needed",
      message_id: PHISH.need?.message_id,
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${PHISH.id}/need/reopen`,
      );
    });
    // Back on the list at once.
    expect(await screen.findByText("This may be phishing. Don’t use its links.")).toBeVisible();
  });

  it("opens an item with Enter: its tells and how Campbooks decided", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "Enter" });
    expect(
      await screen.findByText(
        "The sender’s domain looks like a better-known one (at3lier-sol.example).",
      ),
    ).toBeVisible();
    expect(screen.getByText("How Campbooks decided")).toBeVisible();
  });

  it("opens an item to the email itself, a few lines deep", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // Stepping onto it (past Luz's bill) opens it, as a click does.
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    expect(await screen.findByText(/Hope the move went well/u)).toBeVisible();
    expect(screen.getByRole("button", { name: "Show the whole email" })).toBeVisible();
  });

  it("opens an item to Scout's summary first, then the email", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    const summary = await screen.findByRole("region", { name: "Scout’s summary" });
    expect(summary).toHaveTextContent(
      "Marta is arranging Thursday’s walkthrough with the builders and needs your yes by Wednesday.",
    );
    await screen.findByText(/Hope the move went well/u);
    const text = document.body.textContent;
    expect(text.indexOf("Marta is arranging")).toBeLessThan(
      text.indexOf("Hope the move went well"),
    );
  });

  it("shows no summary when there is none, and the email still shows", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "Enter" });
    expect(await screen.findByText("How Campbooks decided")).toBeVisible();
    expect(screen.queryByRole("region", { name: "Scout’s summary" })).not.toBeInTheDocument();
  });

  it("moves with the arrow keys, opening each item it steps onto, as a click does", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    /** Whether the row holding `text` is open. */
    const isOpen = (text: string) =>
      screen.getByText(text).closest("button")?.getAttribute("aria-expanded") === "true";
    // Nothing is open on arrival.
    expect(isOpen("This may be phishing. Don’t use its links.")).toBe(false);
    // Down Now: from the warning to Luz's bill, a day late.
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(isSelected("Pay Luz Energia")).toBe(true);
    expect(isOpen("Pay Luz Energia")).toBe(true);
    fireEvent.keyDown(document, { key: "ArrowUp" });
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    expect(isOpen("This may be phishing. Don’t use its links.")).toBe(true);
    expect(isOpen("Pay Luz Energia")).toBe(false);
    expect(await screen.findByText("How Campbooks decided")).toBeVisible();
    // Enter closes the open item, Esc too; J and K step and open the same
    // way, from one band into the next.
    fireEvent.keyDown(document, { key: "Enter" });
    expect(isOpen("This may be phishing. Don’t use its links.")).toBe(false);
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    expect(isOpen("Marta Ferreira is waiting for your reply")).toBe(true);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(isOpen("Marta Ferreira is waiting for your reply")).toBe(false);
  });

  it("opens the next item when the open one is done", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "e" });
    expect((await screen.findAllByText("Done · Marta Ferreira")).length).toBeGreaterThan(0);
    // The first of This week takes its place.
    expect(isSelected("Pedro Alves asked you to do something")).toBe(true);
    expect(
      screen
        .getByText("Pedro Alves asked you to do something")
        .closest("button")
        ?.getAttribute("aria-expanded"),
    ).toBe("true");
  });

  it("says how long each item has waited, with the full date on hover", async () => {
    api();
    renderHome();
    const row = (await screen.findByText("Marta Ferreira is waiting for your reply")).closest(
      "button",
    );
    const age = row?.querySelector("time");
    expect(age).not.toBeNull();
    expect(age?.getAttribute("dateTime")).toBe(new Date(MARTA.last_message_at).toISOString());
    expect(age?.getAttribute("title")).toMatch(/2026/u);
    // Every row says it, not only the dated ones.
    const rows = [...document.querySelectorAll("button[id^='attention-']")];
    expect(rows).toHaveLength(NEEDS.length);
    expect(rows.every((r) => r.querySelector("time") !== null)).toBe(true);
  });

  it("shows the selected item's actions in words with keys, and only its", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // The warning: Report and delete, It's safe, then Done and Snooze.
    const warning = screen.getByRole("group", {
      name: "Actions for it-support@at3lier-sol.example",
    });
    expect(
      within(warning)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Report and delete1", "It’s safe2", "DoneE", "Snooze…S"]);
    expect(screen.getAllByRole("group", { name: /^Actions for/u })).toHaveLength(1);
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    const marta = screen.getByRole("group", { name: "Actions for Marta Ferreira" });
    expect(
      within(marta)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Reply1", "DoneE", "Snooze…S", "Not neededX"]);
    // No bare key hints on the rows, and no keyboard legend in the aside.
    const jonas = screen.getByText("Jonas Weber asked you a question").closest("button");
    expect(jonas?.querySelector("kbd")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Keyboard" })).toBeNull();
  });

  it("walks the actions with ← and →, presses one with Enter and clears with Esc", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "ArrowRight" });
    // The row's own name starts with its kind ("Reply · …"): look in its actions.
    const reply = within(
      screen.getByRole("group", { name: "Actions for Marta Ferreira" }),
    ).getByRole("button", { name: /^Reply/u });
    expect(reply).toHaveAttribute("data-highlighted", "true");
    expect(reply).toHaveFocus();
    fireEvent.keyDown(document.activeElement ?? document, { key: "ArrowRight" });
    expect(screen.getByRole("button", { name: /^Done/u })).toHaveFocus();
    fireEvent.keyDown(document.activeElement ?? document, { key: "Escape" });
    expect(screen.getByRole("button", { name: /^Done/u })).not.toHaveAttribute("data-highlighted");
    // From nothing, ← lands on the last one: Not needed.
    fireEvent.keyDown(document, { key: "ArrowLeft" });
    expect(screen.getByRole("button", { name: /^Not needed/u })).toHaveFocus();
    fireEvent.keyDown(document.activeElement ?? document, { key: "Enter" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${MARTA.id}/need/resolve`,
      );
    });
    // The row folds into its confirmation, and Pedro takes the selection.
    expect(screen.getByText("Not needed · Marta Ferreira")).toBeVisible();
    expect(isSelected("Pedro Alves asked you to do something")).toBe(true);
  });

  it("leaves Enter on the toast's Undo to that button", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "x" });
    const undo = await screen.findByRole("button", { name: /^Undo/u });
    undo.focus();
    fireEvent.keyDown(undo, { key: "Enter" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${PHISH.id}/need/reopen`,
      );
    });
    // Luz, now selected, was not touched, nor Marta.
    expect(calls.calls.map((c) => c.route)).not.toContain(
      `POST ${W}/conversations/${MARTA.id}/need/resolve`,
    );
  });

  it("answers a question with 1 and 2", async () => {
    const question = {
      ...JONAS,
      need: JONAS.need === null ? null : { ...JONAS.need, trust: "check" as const },
    };
    const calls = mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: {
        body: {
          conversations: NEEDS.map((c) => (c.id === JONAS.id ? question : c)),
          next_cursor: null,
        },
      },
      [`POST ${W}/conversations/${JONAS.id}/need/resolve`]: { body: { ...JONAS, need: null } },
    });
    renderHome();
    const line = await screen.findByText("Jonas Weber");
    fireEvent.click(line);
    const actions = screen.getByRole("group", { name: "Actions for Jonas Weber" });
    expect(
      within(actions)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Yes1", "No2", "Snooze…S"]);
    // E means nothing until it is answered.
    fireEvent.keyDown(document, { key: "e" });
    expect(calls.calls.some((c) => c.route.endsWith("/need/resolve"))).toBe(false);
    fireEvent.keyDown(document, { key: "2" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toMatchObject({
        outcome: "not_needed",
      });
    });
  });

  it("asks again about an old ask, Still to do?, and No says it is no longer needed", async () => {
    // Pedro's undated lease, three weeks on (campbooks-cloud#275).
    const stale = {
      ...PEDRO,
      need:
        PEDRO.need === null || PEDRO.need === undefined
          ? null
          : {
              ...PEDRO.need,
              reason: "review" as const,
              trust: "check" as const,
              question: "still_to_do" as const,
              due_on: null,
              sent_at: "2026-09-22T10:12:00Z",
            },
    };
    const items = ITEMS.map((item) =>
      item.record.id === PEDRO.id
        ? { ...item, reason: "review" as const, trust: "check" as const, due_on: null }
        : item,
    );
    const calls = mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items, counts: COUNTS } },
      [`GET ${W}/conversations`]: {
        body: {
          conversations: NEEDS.map((c) => (c.id === PEDRO.id ? stale : c)),
          next_cursor: null,
        },
      },
      [`POST ${W}/conversations/${PEDRO.id}/need/resolve`]: { body: { ...PEDRO, need: null } },
    });
    renderHome();
    const line = await screen.findByText("Pedro Alves asked 3 weeks ago. Still to do?");
    fireEvent.click(line);
    const actions = screen.getByRole("group", { name: "Actions for Pedro Alves" });
    expect(
      within(actions)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Yes1", "No2", "Snooze…S"]);
    fireEvent.keyDown(document, { key: "2" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toMatchObject({
        outcome: "not_needed",
      });
    });
    // Nothing is learned from it, so the toast says only that.
    expect(await screen.findByText("No longer needed · Pedro Alves")).toBeVisible();
    expect(screen.queryByText(/Campbooks will learn/u)).toBeNull();
  });

  it("asks whether a bill is paid, and takes Already paid on 1", async () => {
    const calls = mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
      [`POST ${W}/conversations/${ACME.id}/need/resolve`]: { body: { ...ACME, need: null } },
    });
    renderHome();
    expect(await screen.findByText("Acme Hosting sent an invoice. Is it paid?")).toBeVisible();
    // One bill asked about: no "All … already paid".
    expect(screen.queryByRole("button", { name: /already paid/u })).toBeNull();
    fireEvent.click(screen.getByText("Acme Hosting"));
    // Opened, it says why it asks.
    expect(
      screen.getByText("Campbooks can’t see your bank, so it asks whether each invoice is paid."),
    ).toBeVisible();
    const actions = screen.getByRole("group", { name: "Actions for Acme Hosting" });
    expect(
      within(actions)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Already paid1", "Still to pay2", "Snooze…S", "Not neededX"]);
    fireEvent.keyDown(document, { key: "1" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toEqual({
        outcome: "already_paid",
        message_id: ACME.need?.message_id,
      });
    });
    // The row's own confirmation and the toast say it.
    expect(screen.getAllByText("Marked paid · Acme Hosting")).toHaveLength(2);
  });

  it("keeps a bill still to pay as Pay, with a reminder first", async () => {
    const kept = {
      ...ACME,
      need:
        ACME.need === null
          ? null
          : { ...ACME.need, trust: "sure" as const, basis: "member" as const },
    };
    // The server's answer from then on: Still to pay was recorded.
    let answered = false;
    const calls = mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: () => ({
        body: {
          conversations: NEEDS.map((c) => (answered && c.id === ACME.id ? kept : c)),
          next_cursor: null,
        },
      }),
      [`POST ${W}/conversations/${ACME.id}/need/resolve`]: () => {
        answered = true;
        return { body: kept };
      },
    });
    renderHome();
    fireEvent.click(await screen.findByText("Acme Hosting"));
    fireEvent.keyDown(document, { key: "2" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toMatchObject({
        outcome: "still_to_pay",
      });
    });
    expect(await screen.findByText("Pay Acme Hosting")).toBeVisible();
    const actions = screen.getByRole("group", { name: "Actions for Acme Hosting" });
    const labels = within(actions)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(labels[0]).toMatch(/^Remind me /u);
    expect(labels.slice(1)).toEqual(["Mark paid2", "Snooze…S", "Not neededX"]);
    // Luz, overdue, says by how much, in its eyebrow.
    expect(screen.getByText("Pay Luz Energia")).toBeVisible();
    expect(screen.getByText("Overdue · 1 day")).toBeVisible();
  });

  it("marks every bill asked about paid at once, with one Undo", async () => {
    const asked = {
      ...LUZ,
      need: LUZ.need === null ? null : { ...LUZ.need, trust: "check" as const },
    };
    const calls = mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: {
        body: { conversations: NEEDS.map((c) => (c.id === LUZ.id ? asked : c)), next_cursor: null },
      },
      [`POST ${W}/conversations/${ACME.id}/need/resolve`]: { body: { ...ACME, need: null } },
      [`POST ${W}/conversations/${LUZ.id}/need/resolve`]: { body: { ...LUZ, need: null } },
      [`POST ${W}/conversations/${ACME.id}/need/reopen`]: { body: ACME },
      [`POST ${W}/conversations/${LUZ.id}/need/reopen`]: { body: asked },
    });
    renderHome();
    fireEvent.click(await screen.findByRole("button", { name: /^All 2 already paid/u }));
    await waitFor(() => {
      expect(
        calls.calls.filter((c) => c.route.endsWith("/need/resolve")).map((c) => c.body),
      ).toHaveLength(2);
    });
    expect(await screen.findByText("Marked 2 invoices paid")).toBeVisible();
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.filter((c) => c.route.endsWith("/need/reopen"))).toHaveLength(2);
    });
  });

  it("opens the reply dialog with R; Write my own opens the reader's composer", async () => {
    const calls = api();
    const { router } = renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "r" });
    expect(await screen.findByRole("dialog", { name: "Reply to Marta" })).toBeVisible();
    expect(await screen.findByRole("button", { name: /Say Thursday works/u })).toBeVisible();
    // Scout reads the newest message; nothing is written before a pick.
    expect(
      calls.calls.find((c) => c.route === `POST ${W}/conversations/${MARTA.id}/reply/suggestions`)
        ?.body,
    ).toEqual({ message_id: MARTA_DETAIL.messages.at(-1)?.id });
    expect(calls.calls.some((c) => c.route.endsWith("/reply/draft/stream"))).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: /Write my own/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/primary/${MARTA.id}`);
    });
    expect(router.state.location.search).toEqual({ reply: true });
  });

  it("sends Scout's draft from the dialog and folds the item", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "1" });
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(first, { key: "1" });
    const text = await screen.findByRole("textbox", { name: "Your reply" });
    expect(text).toHaveValue(MARTA_REPLY);
    // ⌘↵ is bound in an effect that runs after the draft is on screen, and
    // Testing Library hands the textbox back as soon as it renders: under
    // load, a press in between finds nothing bound and is lost
    // (campbooks-cloud#245). Press until the send has started, never again
    // once it has (a press while it is under way is ignored anyway).
    const started = () => calls.calls.some((c) => c.route === `POST ${W}/drafts`);
    await waitFor(() => {
      if (!started()) {
        fireEvent.keyDown(text, { key: "Enter", ctrlKey: true });
      }
      expect(started()).toBe(true);
    });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(`POST ${W}/drafts/${MARTA_DRAFT.id}/send`);
    });
    expect(calls.calls.filter((c) => c.route === `POST ${W}/drafts`)).toHaveLength(1);
    expect(calls.calls.find((c) => c.route === `POST ${W}/drafts`)?.body).toMatchObject({
      kind: "reply",
      body: MARTA_REPLY,
    });
    expect((await screen.findAllByText("Sending to Marta Ferreira")).length).toBeGreaterThan(0);
  });

  it("snoozes an item until Monday from the Snooze dialog (S)", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("Pedro Alves asked you to do something");
    for (const key of ["j", "j", "j"]) {
      fireEvent.keyDown(document, { key });
    }
    fireEvent.keyDown(document, { key: "s" });
    const dialog = await screen.findByRole("dialog", { name: "Snooze Pedro Alves until…" });
    expect(within(dialog).getAllByRole("button", { name: /morning|today|week/u }).length).toBe(4);
    fireEvent.click(within(dialog).getByRole("button", { name: /Monday morning/u }));
    await waitFor(() => {
      expect(screen.queryByText("Pedro Alves asked you to do something")).toBeNull();
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    const snooze = calls.calls.find((c) => c.route.endsWith("/snooze"));
    expect(new Date((snooze?.body as { until: string }).until).getDay()).toBe(1);
    expect(PEDRO.need?.reason).toBe("do");
  });

  it("closes the Snooze dialog with Esc and changes nothing", async () => {
    const calls = api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "s" });
    const dialog = await screen.findByRole("dialog");
    fireEvent.keyDown(within(dialog).getAllByRole("button")[1] ?? dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText("This may be phishing. Don’t use its links.")).toBeVisible();
    expect(calls.calls.some((c) => c.route.endsWith("/snooze"))).toBe(false);
  });

  it("says a failed snooze until Retry or dismiss, and puts the item back", async () => {
    mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
      [`POST ${W}/attention/${ITEMS[0]?.id ?? ""}/snooze`]: problem(503, "unavailable"),
    });
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "s" });
    fireEvent.keyDown(await screen.findByRole("dialog"), { key: "2" });
    expect(await screen.findByRole("alert")).toHaveTextContent(/Couldn’t snooze/u);
    expect(screen.getByRole("button", { name: "Retry" })).toBeVisible();
    expect(await screen.findByText("This may be phishing. Don’t use its links.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps the selection when a live update takes the selected item away", async () => {
    const live = { items: ITEMS };
    const calls = api(ITEMS, live);
    const { queryClient } = renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // The first item is selected from the start; J J moves past Luz to Marta.
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    // Marta is replied to elsewhere: the live update drops her.
    live.items = ITEMS.filter((item) => item.record.id !== MARTA.id);
    await queryClient.invalidateQueries();
    await waitFor(() => {
      expect(screen.queryByText("Marta Ferreira is waiting for your reply")).toBeNull();
    });
    // Her neighbour takes the selection, not the top of the list.
    expect(isSelected("Pedro Alves asked you to do something")).toBe(true);
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${PEDRO.id}/need/resolve`,
      );
    });
    expect(calls.calls.map((c) => c.route)).not.toContain(
      `POST ${W}/conversations/${PHISH.id}/need/resolve`,
    );
  });

  it("keeps the confirmation in place when the list refetches without the item", async () => {
    const live = { items: ITEMS };
    api(ITEMS, live);
    const { queryClient } = renderHome();
    await screen.findByText("Marta Ferreira is waiting for your reply");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "j" });
    // The server answers and the list no longer has Marta before the row has folded.
    live.items = ITEMS.filter((item) => item.record.id !== MARTA.id);
    fireEvent.keyDown(document, { key: "e" });
    await queryClient.invalidateQueries();
    // The row's own confirmation, next to the toast saying the same.
    expect(screen.getAllByText("Done · Marta Ferreira")).toHaveLength(2);
    expect(screen.queryByText("Marta Ferreira is waiting for your reply")).toBeNull();
  });

  it("says a failure even when a second action follows at once, and drops it from Z", async () => {
    const calls = mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
      [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: problem(503, "unavailable"),
      [`POST ${W}/conversations/${LUZ.id}/need/resolve`]: { body: { ...LUZ, need: null } },
      [`POST ${W}/conversations/${LUZ.id}/need/reopen`]: { body: LUZ },
    });
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // X on the warning, then X (Not needed) on Luz's bill, which took its
    // place, before the first request comes back.
    fireEvent.keyDown(document, { key: "x" });
    fireEvent.keyDown(document, { key: "x" });
    expect(await screen.findByRole("alert")).toHaveTextContent(/nothing changed/u);
    expect(await screen.findByText("This may be phishing. Don’t use its links.")).toBeVisible();
    // Z takes back Luz's Not needed; the failed verdict is not in the
    // history, so a second Z reopens nothing.
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${LUZ.id}/need/reopen`,
      );
    });
    fireEvent.keyDown(document, { key: "z" });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(calls.calls.map((c) => c.route)).not.toContain(
      `POST ${W}/conversations/${PHISH.id}/need/reopen`,
    );
  });

  it("puts the item back and says so when the need changed meanwhile", async () => {
    mockApi({
      [`GET ${W}/attention`]: (request) =>
        new URL(request.url).searchParams.get("state") === "cleared"
          ? { body: { items: [], counts: [] } }
          : { body: { items: ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: NEEDS, next_cursor: null } },
      [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: problem(409, "need_changed"),
    });
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "e" });
    expect(await screen.findByText(/changed while you looked/u)).toBeVisible();
    expect(screen.getByText("This may be phishing. Don’t use its links.")).toBeVisible();
  });

  it("shows mail a rule keeps in front of you in Whenever, as Worth a look, with Seen and Snooze", async () => {
    const calls = api([...ITEMS, DIGEST_ITEM]);
    renderHome();
    await screen.findByText("You asked to see mail from Design Notes");
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 4)).toEqual(["Now 2", "Today 1", "This week 2", "Whenever 2"]);
    expect(screen.getByText("Kept here by your rule")).toBeVisible();
    // It asks nothing of you: the heading counts the others only.
    expect(screen.getByRole("heading", { level: 1, name: "6 things need you" })).toBeVisible();
    // Nothing is asked of you: no Reply, and no "Not needed" about your own rule.
    fireEvent.click(screen.getByText("You asked to see mail from Design Notes"));
    const row = screen.getByRole("group", { name: "Actions for Design Notes" });
    expect(
      within(row)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["SeenE", "Snooze…S"]);
    fireEvent.click(within(row).getByRole("button", { name: /^Seen/u }));
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route === `POST ${W}/conversations/${DIGEST.id}/need/resolve`)
          ?.body,
      ).toEqual({ outcome: "done", message_id: DIGEST.need?.message_id });
    });
    expect(await screen.findByText("Seen · Design Notes")).toBeVisible();
  });

  it("a Home with only Worth a look says nothing needs you, and still shows what is worth a look", async () => {
    api([DIGEST_ITEM]);
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Nothing needs you" }),
    ).toBeVisible();
    expect(
      screen.getByText("Nothing asks anything of you. 1 email below is worth a look."),
    ).toBeVisible();
    expect(await screen.findByText("You asked to see mail from Design Notes")).toBeVisible();
    expect(screen.getByRole("heading", { level: 2, name: "Whenever 1" })).toBeVisible();
  });

  describe("Scout noticed", () => {
    const NOTICE = {
      id: "0192f1a0-0000-7000-8000-0000000000e1",
      inbox_id: "0192f1a0-0000-7000-8000-000000000001",
      kind: "archive_on_arrival",
      sender: "digest@design-notes.example",
      sender_name: "Design Notes",
      conditions: [{ field: "sender_address", value: "digest@design-notes.example" }],
      actions: [{ action: "archive" }],
      evidence: {
        received: 88,
        opened: 0,
        archived_unread: 88,
        last_week: 7,
        window_days: 60,
        cadence: "daily",
      },
      shared: false,
    };
    const notices = { [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } } };

    it("sits right after the first group, not after all of them", async () => {
      api(ITEMS, undefined, notices);
      renderHome();
      const note = await screen.findByRole("region", { name: "Scout noticed" });
      const heading = (name: string) =>
        screen.getAllByRole("heading", { level: 2 }).find((h) => h.textContent === name);
      const review = heading("Now 2");
      const reply = heading("Today 1");
      const pay = heading("This week 2");
      const follows = (a: Node | undefined, b: Node) =>
        a !== undefined && (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
      expect(follows(review, note)).toBe(true);
      expect(follows(note, reply as Node)).toBe(true);
      expect(follows(note, pay as Node)).toBe(true);
      expect(screen.getAllByRole("region", { name: "Scout noticed" })).toHaveLength(1);
    });

    it("keeps its place when nothing needs you", async () => {
      api([], undefined, notices);
      renderHome();
      expect(await screen.findByRole("region", { name: "Scout noticed" })).toBeVisible();
      expect(screen.getByRole("heading", { level: 1, name: "Nothing needs you" })).toBeVisible();
    });
  });

  it("says when nothing needs you", async () => {
    api([]);
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Nothing needs you" }),
    ).toBeVisible();
  });

  it("explains when it cannot load", async () => {
    mockApi({
      [`GET ${W}/attention`]: problem(503, "unavailable"),
      [`GET ${W}/conversations`]: { body: { conversations: [], next_cursor: null } },
    });
    renderHome();
    expect(await screen.findByText("Could not load what needs you")).toBeVisible();
  });
});
