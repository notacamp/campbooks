// Home's Nudge group and Waiting on others (campbooks-cloud#293): who has
// not written back yet, the nudge Scout drafts, and who Scout waits on.
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Schema } from "~/lib/api";

import {
  ANA,
  ANA_DETAIL,
  ANA_ITEM,
  CLEARED,
  INES,
  ITEMS,
  LUZ,
  MARTA,
  RUI,
  RUI_DETAIL,
  SOFIA,
  WAITING,
} from "../../../../../test/demo/attention";
import { INBOX, NOW, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

const MARTA_ITEM = ITEMS.find((item) => item.record.id === MARTA.id);
const LUZ_ITEM = ITEMS.find((item) => item.record.id === LUZ.id);
if (MARTA_ITEM === undefined || LUZ_ITEM === undefined) {
  throw new Error("the fixtures lost Marta or Luz");
}
/** A reply, a bill and a nudge, in the attention API's order. */
const OPEN = [MARTA_ITEM, LUZ_ITEM, ANA_ITEM];

const TEXT = "Hi Ana,\n\nCould you send me the revised floor plan before Thursday?\n\nThanks,\nSam";
const NUDGE: Schema<"DraftView"> = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-000000000201",
  kind: "nudge",
  conversation_id: ANA.id,
  message_id: ANA.need?.message_id ?? null,
  to: [{ name: "Ana Costa", address: "ana@costa-arquitetos.example" }],
  cc: [{ name: "João Matos", address: "joao@obras-matos.example" }],
  subject: `Re: ${ANA.subject}`,
};

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
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

function api({ role = "manager" }: { role?: "manager" | "reader" } = {}) {
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role }] } },
    [`GET ${W}/attention`]: (request) => {
      const state = new URL(request.url).searchParams.get("state");
      if (state === "cleared") {
        return { body: { items: CLEARED, counts: [] } };
      }
      if (state === "waiting") {
        return { body: { items: WAITING, counts: [{ reason: "waiting", count: 2 }] } };
      }
      return { body: { items: OPEN, counts: [] } };
    },
    [`GET ${W}/conversations`]: {
      body: { conversations: [MARTA, LUZ, ANA, RUI, INES, SOFIA], next_cursor: null },
    },
    [`GET ${W}/conversations/${ANA.id}`]: { body: ANA_DETAIL },
    [`GET ${W}/conversations/${RUI.id}`]: { body: RUI_DETAIL },
    [`POST ${W}/conversations/${ANA.id}/need/resolve`]: { body: { ...ANA, need: null } },
    [`POST ${W}/conversations/${ANA.id}/need/reopen`]: { body: ANA },
    [`POST ${W}/conversations/${INES.id}/need/resolve`]: { body: { ...INES, need: null } },
    [`POST ${W}/conversations/${INES.id}/need/reopen`]: { body: INES },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`POST ${W}/conversations/${ANA.id}/nudge/draft`]: {
      body: { message_id: ANA.need?.message_id, text: TEXT },
    },
    [`POST ${W}/conversations/${RUI.id}/nudge/draft`]: {
      body: { message_id: RUI.need?.message_id, text: "Hi Rui,\n\nAny news on the contract?" },
    },
    [`POST ${W}/drafts`]: { status: 201, body: NUDGE },
    [`POST ${W}/drafts/${NUDGE.id}/send`]: {
      status: 202,
      body: { ...NUDGE, body: TEXT, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
    },
    [`POST ${W}/drafts/${NUDGE.id}/cancel_send`]: { body: { ...NUDGE, body: TEXT, version: 3 } },
  });
}

/** Select Ana's row: Marta's and Luz's come first. */
async function toAna() {
  await screen.findByText("Ana Costa hasn’t written yet");
  fireEvent.keyDown(document, { key: "j" });
  fireEvent.keyDown(document, { key: "j" });
  return within(await screen.findByRole("group", { name: "Actions for Ana Costa" }));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("What needs you: Nudge", () => {
  it("lists the nudge after the bills, saying who hasn't written and since when", async () => {
    api();
    renderHome();
    expect(
      await screen.findByRole("heading", { level: 1, name: "3 things need you" }),
    ).toBeVisible();
    const groups = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(groups.slice(0, 3)).toEqual(["Reply 1", "Pay 1", "Nudge 1"]);
    expect(screen.getByText("Ana Costa hasn’t written yet")).toBeVisible();
    // João's line, quoted; asked on 7 October.
    expect(
      screen.getByText(
        "Ana, could you send Sam the revised floor plan directly, before Thursday’s visit?",
      ),
    ).toBeVisible();
    expect(screen.getByText(/^Asked (7 Oct|Oct 7)$/u)).toBeVisible();
    // Waits never count in what needs you; the hero says Scout waits too.
    expect(screen.getByText(/Scout is also waiting on 2 people for you\./u)).toBeVisible();
  });

  it("offers Nudge on 1, marked with the visor, then Heard from them, Snooze and Not waiting", async () => {
    api();
    renderHome();
    const row = await toAna();
    const labels = row.getAllByRole("button").map((b) => b.textContent);
    expect(labels[0]).toMatch(/^Nudge Ana…/u);
    expect(labels[1]).toMatch(/^Heard from them/u);
    expect(labels[2]).toMatch(/^Snooze…/u);
    expect(labels[3]).toMatch(/^Not waiting/u);
  });

  it("nudges with 1: Scout's text, Send, the item folds away, and Undo brings both back", async () => {
    const calls = api();
    renderHome();
    await toAna();
    fireEvent.keyDown(document, { key: "1" });
    expect(await screen.findByRole("dialog", { name: "Nudge Ana" })).toBeVisible();
    const text = await screen.findByRole("textbox", { name: "Your nudge" });
    await waitFor(() => {
      expect(text).toHaveValue(TEXT);
    });
    const send = screen.getByRole("button", { name: /^Send/u });
    await waitFor(() => {
      expect(send).toBeEnabled();
    });
    fireEvent.click(send);
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(`POST ${W}/drafts/${NUDGE.id}/send`);
    });
    expect((await screen.findAllByText("Nudged Ana")).length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(screen.queryByText("Ana Costa hasn’t written yet")).toBeNull();
    });

    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(`POST ${W}/drafts/${NUDGE.id}/cancel_send`);
    });
    // The item and the dialog are back, on the same draft.
    expect(await screen.findByText("Ana Costa hasn’t written yet")).toBeVisible();
    expect(await screen.findByRole("textbox", { name: "Your nudge" })).toHaveValue(TEXT);
    expect(
      calls.calls.filter((c) => c.route === `POST ${W}/conversations/${ANA.id}/nudge/draft`),
    ).toHaveLength(1);
  });

  it("marks that they wrote with E, and takes it back with Z", async () => {
    const calls = api();
    renderHome();
    await toAna();
    fireEvent.keyDown(document, { key: "e" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route === `POST ${W}/conversations/${ANA.id}/need/resolve`)?.body,
      ).toEqual({ outcome: "done", message_id: ANA.need?.message_id });
    });
    expect(await screen.findByText("Heard from Ana Costa · Scout stopped waiting")).toBeVisible();
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(
        `POST ${W}/conversations/${ANA.id}/need/reopen`,
      );
    });
  });

  it("stops waiting with X, saying so, with Undo", async () => {
    const calls = api();
    renderHome();
    await toAna();
    fireEvent.keyDown(document, { key: "x" });
    await waitFor(() => {
      expect(
        calls.calls.find((c) => c.route === `POST ${W}/conversations/${ANA.id}/need/resolve`)?.body,
      ).toEqual({ outcome: "not_needed", message_id: ANA.need?.message_id });
    });
    expect((await screen.findAllByText("Not waiting on Ana Costa")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /^Undo/u })).toBeVisible();
  });

  it("puts it off with S", async () => {
    api();
    renderHome();
    await toAna();
    fireEvent.keyDown(document, { key: "s" });
    expect(await screen.findByRole("dialog", { name: "Snooze Ana Costa until…" })).toBeVisible();
  });

  it("offers only Snooze in an inbox the member may only read", async () => {
    api({ role: "reader" });
    renderHome();
    const row = await toAna();
    await waitFor(() => {
      expect(row.getAllByRole("button").map((b) => b.textContent)).toEqual([
        expect.stringMatching(/^Snooze…/u),
      ]);
    });
  });
});

describe("What needs you: Waiting on others", () => {
  it("lists who Scout waits on, soonest check first, outside the count", async () => {
    api();
    renderHome();
    const list = await screen.findByRole("heading", { level: 2, name: "Waiting on others" });
    expect(list).toBeVisible();
    const rows = screen
      .getAllByRole("listitem")
      .map((li) => li.getAttribute("aria-label"))
      .filter((label) => label !== null);
    expect(rows).toEqual(["Waiting on Rui Matos", "Waiting on Inês Cardoso"]);
    const rui = within(screen.getByRole("listitem", { name: "Waiting on Rui Matos" }));
    expect(rui.getByText("Checks Fri")).toBeVisible();
    const ines = within(screen.getByRole("listitem", { name: "Waiting on Inês Cardoso" }));
    expect(ines.getByText("Checks Mon")).toBeVisible();
    // Each part isolated from the page's text direction.
    expect(ines.getByText("Nudged once").tagName).toBe("BDI");
    expect(rui.getByText("Rui Matos").tagName).toBe("BDI");
    expect(screen.getByRole("heading", { level: 1, name: "3 things need you" })).toBeVisible();
  });

  it("opens the nudge dialog for Rui with Nudge now", async () => {
    const calls = api();
    renderHome();
    const rui = within(await screen.findByRole("listitem", { name: "Waiting on Rui Matos" }));
    fireEvent.click(rui.getByRole("button", { name: /Nudge now/u }));
    expect(await screen.findByRole("dialog", { name: "Nudge Rui" })).toBeVisible();
    expect(await screen.findByRole("textbox", { name: "Your nudge" })).toHaveValue(
      "Hi Rui,\n\nAny news on the contract?",
    );
    expect(calls.calls.map((c) => c.route)).toContain(
      `POST ${W}/conversations/${RUI.id}/nudge/draft`,
    );
    // Rui wrote the email, so the nudge copies nobody.
    const to = await screen.findByText("Rui Matos <rui@matos-carpintaria.example>");
    expect(to.closest("p")).toHaveTextContent("To Rui Matos <rui@matos-carpintaria.example>");
    expect(screen.getByRole("dialog")).not.toHaveTextContent("Copying");
  });

  it("stops waiting on Inês with Not waiting, and Undo brings the row back", async () => {
    const calls = api();
    renderHome();
    const ines = within(await screen.findByRole("listitem", { name: "Waiting on Inês Cardoso" }));
    fireEvent.click(ines.getByRole("button", { name: /Not waiting/u }));
    await waitFor(() => {
      expect(screen.queryByRole("listitem", { name: "Waiting on Inês Cardoso" })).toBeNull();
    });
    expect(
      calls.calls.find((c) => c.route === `POST ${W}/conversations/${INES.id}/need/resolve`)?.body,
    ).toEqual({ outcome: "not_needed", message_id: INES.need?.message_id });
    expect(await screen.findByText("Not waiting on Inês Cardoso")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /^Undo/u }));
    expect(await screen.findByRole("listitem", { name: "Waiting on Inês Cardoso" })).toBeVisible();
  });

  it("is a fold after the list below 48rem, with its count", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("48rem"),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    api();
    renderHome();
    const fold = await screen.findByText("Waiting on others · 2");
    expect(fold.closest("details")).not.toBeNull();
    expect(screen.queryByRole("heading", { level: 2, name: "Waiting on others" })).toBeNull();
    expect(screen.getByRole("listitem", { name: "Waiting on Rui Matos" })).toBeInTheDocument();
  });
});
