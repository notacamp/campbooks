// Home's question when Scout is unsure whose move it is (campbooks-cloud#372): Ana Costa
// wrote to Águas do Ribatejo, copying Sam. Nudge them, Reply to Ana, or Nothing for me.
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
  CLEARED,
  COPIED,
  COPIED_DETAIL,
  COPIED_ITEM,
  COPIED_WAITING,
  ITEMS,
  MARTA,
} from "../../../../../test/demo/attention";
import { INBOX, NOW, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

const MARTA_ITEM = ITEMS.find((item) => item.record.id === MARTA.id);
if (MARTA_ITEM === undefined) {
  throw new Error("the fixtures lost Marta");
}

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
    component: () => <p>Reader</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([home, conversation]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return renderWithProviders(<RouterProvider router={router} />);
}

function api({
  role = "manager",
  party = true,
}: { role?: "manager" | "reader"; party?: boolean } = {}) {
  const copied = party
    ? COPIED
    : { ...COPIED, need: { ...COPIED.need, unsure: { ...COPIED.need?.unsure, party: null } } };
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role }] } },
    [`GET ${W}/attention`]: (request) => {
      const state = new URL(request.url).searchParams.get("state");
      if (state === "cleared") {
        return { body: { items: CLEARED, counts: [] } };
      }
      return { body: { items: state === "waiting" ? [] : [COPIED_ITEM, MARTA_ITEM], counts: [] } };
    },
    [`GET ${W}/conversations`]: { body: { conversations: [copied, MARTA], next_cursor: null } },
    [`GET ${W}/conversations/${COPIED.id}`]: { body: COPIED_DETAIL },
    // Someone else has the move: it waits on them. Anything else: it needs nothing.
    [`POST ${W}/conversations/${COPIED.id}/need/resolve`]: (_request, body) => ({
      body:
        typeof body === "object" && body !== null && "wait_on" in body
          ? COPIED_WAITING
          : { ...COPIED, need: null },
    }),
    [`POST ${W}/conversations/${COPIED.id}/need/confirm`]: {
      body: { ...COPIED, need: { ...COPIED.need, trust: "sure", basis: "member" } },
    },
    [`POST ${W}/conversations/${COPIED.id}/need/reopen`]: { body: COPIED },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`POST ${W}/conversations/${COPIED.id}/nudge/draft`]: {
      body: {
        message_id: COPIED.need?.message_id,
        text: "Hello,\n\nCould you confirm the meter reading?\n\nThanks,\nSam",
      },
    },
    [`POST ${W}/conversations/${COPIED.id}/reply/suggestions`]: {
      body: { message_id: COPIED.need?.message_id, intents: ["Yes, I will confirm it"] },
    },
  });
}

/** Select the unsure row (a click, as the stories do): its actions show under it. */
async function selected(name = "Actions for Ana Costa") {
  fireEvent.click(await screen.findByText("Ana Costa wrote to Águas do Ribatejo. You’re in Cc."));
  return within(await screen.findByRole("group", { name }));
}

describe("What needs you: Scout isn't sure", () => {
  it("says who wrote to whom, that they haven't answered and that Scout can't tell", async () => {
    api();
    renderHome();
    expect(
      await screen.findByText("Ana Costa wrote to Águas do Ribatejo. You’re in Cc."),
    ).toBeVisible();
    expect(
      screen.getByText(
        "They haven’t answered in 9 days. Scout can’t tell whose move this is. What’s next for you?",
      ),
    ).toBeVisible();
    // Its kind is the chip.
    expect(screen.getByText("Scout isn’t sure")).toBeVisible();
  });

  it("offers Nudge, Reply and Nothing for me on 1 to 3, then Later", async () => {
    api();
    renderHome();
    const row = await selected();
    const labels = row.getAllByRole("button").map((b) => b.textContent);
    expect(labels[0]).toMatch(/^Nudge Águas…/u);
    expect(labels[1]).toMatch(/^Reply to Ana…/u);
    expect(labels[2]).toMatch(/^Nothing for me/u);
    expect(labels[3]).toMatch(/^Snooze…/u);
    // Yes and No are not asked: this is not that question.
    expect(row.queryByRole("button", { name: /^Yes/u })).toBeNull();
  });

  it("hides Nudge when nobody is the natural one to nudge", async () => {
    api({ party: false });
    renderHome();
    const row = await selected();
    const labels = row.getAllByRole("button").map((b) => b.textContent);
    expect(labels[0]).toMatch(/^Reply to Ana…/u);
    expect(labels[1]).toMatch(/^Nothing for me/u);
    expect(labels.some((label) => label.startsWith("Nudge"))).toBe(false);
  });

  it("offers nothing to write in an inbox the member only reads", async () => {
    api({ role: "reader" });
    renderHome();
    const row = await selected();
    const labels = row.getAllByRole("button").map((b) => b.textContent);
    expect(labels.some((label) => /^(Nudge|Reply|Nothing)/u.test(label))).toBe(false);
    expect(labels[0]).toMatch(/^Snooze…/u);
  });

  it("puts the move on the person with 1: the row folds, the nudge is offered, Undo takes it back", async () => {
    const calls = api();
    renderHome();
    await selected();
    fireEvent.keyDown(document, { key: "1" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route === `POST ${W}/conversations/${COPIED.id}/need/resolve`)
          ?.body,
      ).toEqual({
        outcome: "not_needed",
        message_id: COPIED.need?.message_id,
        wait_on: "geral@aguas-ribatejo.example",
      });
    });
    expect((await screen.findAllByText("Waiting on Águas do Ribatejo")).length).toBeGreaterThan(0);
    // The nudge to them is a dialog away, and optional.
    expect(await screen.findByRole("dialog", { name: "Nudge Águas" })).toBeVisible();
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Nudge Águas" })).toBeNull();
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${COPIED.id}/need/reopen`,
      );
    });
  });

  it("says Yes before the reply dialog with 2", async () => {
    const calls = api();
    renderHome();
    await selected();
    fireEvent.keyDown(document, { key: "2" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route === `POST ${W}/conversations/${COPIED.id}/need/confirm`)
          ?.body,
      ).toEqual({ message_id: COPIED.need?.message_id });
    });
    expect(await screen.findByRole("dialog", { name: /^Reply to Ana/u })).toBeVisible();
  });

  it("says nothing is needed with 3", async () => {
    const calls = api();
    renderHome();
    await selected();
    fireEvent.keyDown(document, { key: "3" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route === `POST ${W}/conversations/${COPIED.id}/need/resolve`)
          ?.body,
      ).toEqual({ outcome: "not_needed", message_id: COPIED.need?.message_id });
    });
    expect((await screen.findAllByText("Not needed · Ana Costa")).length).toBeGreaterThan(0);
  });
});
