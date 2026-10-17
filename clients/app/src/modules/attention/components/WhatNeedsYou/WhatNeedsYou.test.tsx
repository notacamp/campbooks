import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  ACME,
  CLEARED,
  COUNTS,
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
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

function renderHome() {
  const root = createRootRoute({ component: Outlet });
  const home = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => <WhatNeedsYou workspaceId={DUARTE.id} now={NOW} />,
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

function api(items = ITEMS, live?: { items: typeof ITEMS }) {
  return mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: live?.items ?? items, counts: COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations: [...NEEDS, SOFIA], next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: { body: { ...PHISH, need: null } },
    [`POST ${W}/conversations/${PHISH.id}/need/reopen`]: { body: PHISH },
    [`POST ${W}/conversations/${MARTA.id}/need/resolve`]: { body: { ...MARTA, need: null } },
    [`POST ${W}/conversations/${JONAS.id}/need/resolve`]: { body: { ...JONAS, need: null } },
    [`POST ${W}/attention/${ITEMS[3]?.id ?? ""}/snooze`]: { body: ITEMS[3] },
    [`POST ${W}/conversations/${MARTA.id}/reply/suggestions`]: {
      body: {
        message_id: MARTA_DETAIL.messages.at(-1)?.id,
        intents: ["Say Thursday works", "Ask to move it"],
      },
    },
    [`POST ${W}/conversations/${MARTA.id}/reply/draft`]: {
      body: { message_id: MARTA_DETAIL.messages.at(-1)?.id, text: MARTA_REPLY },
    },
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
  it("groups what needs you, Review first, in plain words", async () => {
    api();
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 4)).toEqual(["Review 1", "Reply 2", "Do 1", "Pay 2"]);
    expect(screen.getByText("Marta Ferreira is waiting for your reply")).toBeVisible();
    expect(screen.getByText("Due tomorrow")).toBeVisible();
    // The warning names the sender by address; no percentage anywhere.
    expect(screen.getByText("it-support@at3lier-sol.example")).toBeVisible();
    expect(document.body).not.toHaveTextContent("%");
    expect(screen.getByText("Replied · Tile samples")).toBeVisible();
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
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "Enter" });
    expect(await screen.findByText(/Hope the move went well/u)).toBeVisible();
    expect(screen.getByRole("button", { name: "Show the whole email" })).toBeVisible();
  });

  it("opens an item to Scout's summary first, then the email", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "j" });
    fireEvent.keyDown(document, { key: "Enter" });
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

  it("moves with the arrow keys, even with an item open", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    fireEvent.keyDown(document, { key: "ArrowUp" });
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "Enter" });
    await screen.findByText("How Campbooks decided");
    // Open: ↓ still steps (and closes it), as do J and K.
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    expect(screen.queryByText("How Campbooks decided")).toBeNull();
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Jonas Weber asked you a question")).toBe(true);
  });

  it("shows the selected item's actions in words with keys, and only its", async () => {
    api();
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // The warning: It's safe, then Done and Snooze.
    const warning = screen.getByRole("group", {
      name: "Actions for it-support@at3lier-sol.example",
    });
    expect(
      within(warning)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["It’s safe1", "DoneE", "Snooze…S"]);
    expect(screen.getAllByRole("group", { name: /^Actions for/u })).toHaveLength(1);
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
    fireEvent.keyDown(document, { key: "ArrowRight" });
    const reply = screen.getByRole("button", { name: /^Reply/u });
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
    // The row folds into its confirmation, and Jonas takes the selection.
    expect(screen.getByText("Not needed · Marta Ferreira")).toBeVisible();
    expect(isSelected("Jonas Weber asked you a question")).toBe(true);
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
    // Marta, now selected, was not touched.
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
    expect(
      screen.getByText("Campbooks can’t see your bank, so it asks whether each invoice is paid."),
    ).toBeVisible();
    // One bill asked about: no "All … already paid".
    expect(screen.queryByRole("button", { name: /already paid/u })).toBeNull();
    fireEvent.click(screen.getByText("Acme Hosting"));
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
    // Luz, overdue, says by how much.
    expect(screen.getByText("Pay Luz Energia")).toBeVisible();
    expect(screen.getByText("1 day overdue")).toBeVisible();
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
    fireEvent.keyDown(document, { key: "r" });
    expect(await screen.findByRole("dialog", { name: "Reply to Marta" })).toBeVisible();
    expect(await screen.findByRole("button", { name: /Say Thursday works/u })).toBeVisible();
    // Scout reads the newest message; nothing is written before a pick.
    expect(
      calls.calls.find((c) => c.route === `POST ${W}/conversations/${MARTA.id}/reply/suggestions`)
        ?.body,
    ).toEqual({ message_id: MARTA_DETAIL.messages.at(-1)?.id });
    expect(calls.calls.some((c) => c.route.endsWith("/reply/draft"))).toBe(false);
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
    fireEvent.keyDown(document, { key: "1" });
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(first, { key: "1" });
    const text = await screen.findByRole("textbox", { name: "Your reply" });
    expect(text).toHaveValue(MARTA_REPLY);
    fireEvent.keyDown(text, { key: "Enter", ctrlKey: true });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(`POST ${W}/drafts/${MARTA_DRAFT.id}/send`);
    });
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
    // The first item is selected from the start; J moves to Marta.
    expect(isSelected("This may be phishing. Don’t use its links.")).toBe(true);
    fireEvent.keyDown(document, { key: "j" });
    expect(isSelected("Marta Ferreira is waiting for your reply")).toBe(true);
    // Marta is replied to elsewhere: the live update drops her.
    live.items = ITEMS.filter((item) => item.record.id !== MARTA.id);
    await queryClient.invalidateQueries();
    await waitFor(() => {
      expect(screen.queryByText("Marta Ferreira is waiting for your reply")).toBeNull();
    });
    // Her neighbour takes the selection, not the top of the list.
    expect(isSelected("Jonas Weber asked you a question")).toBe(true);
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${JONAS.id}/need/resolve`,
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
      [`POST ${W}/conversations/${MARTA.id}/need/resolve`]: { body: { ...MARTA, need: null } },
      [`POST ${W}/conversations/${MARTA.id}/need/reopen`]: { body: MARTA },
    });
    renderHome();
    await screen.findByText("This may be phishing. Don’t use its links.");
    // X on the warning, then E on the row that took its place, before the
    // first request comes back.
    fireEvent.keyDown(document, { key: "x" });
    fireEvent.keyDown(document, { key: "e" });
    expect(await screen.findByRole("alert")).toHaveTextContent(/nothing changed/u);
    expect(await screen.findByText("This may be phishing. Don’t use its links.")).toBeVisible();
    // Z takes back Marta's Done; the failed verdict is not in the history,
    // so a second Z reopens nothing.
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${MARTA.id}/need/reopen`,
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
