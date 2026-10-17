// Home's ranked actions (campbooks-cloud#166): the server's `actions` on a
// need replace the row's fixed own actions; a missing key keeps them.
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

import type { Schema } from "~/lib/api";

import {
  CLEARED,
  COUNTS,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
  PHISH,
  PHISH_DETAIL,
  SOFIA,
} from "../../../../../test/demo/attention";
import { NOW, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WhatNeedsYou } from "./WhatNeedsYou";

type Summary = Schema<"ConversationSummary">;
type Suggested = Schema<"SuggestedAction">[];

const PICKED = "Scout picked these for this email";
/** A warning names its sender by address. */
const PHISH_NAME = "it-support@at3lier-sol.example";

/** `summary` with the server's ranked actions on its need. */
function ranked(summary: Summary, actions: Suggested): Summary {
  return summary.need === null || summary.need === undefined
    ? summary
    : { ...summary, need: { ...summary.need, actions } };
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
  return { router, ...renderWithProviders(<RouterProvider router={router} />) };
}

const PEOPLE: Schema<"SenderView">[] = [
  { address: "rita@silva-contas.example", name: "Rita Gomes", received: 12, replied: 9 },
  { address: "joao@obras-matos.example", name: "João Matos", received: 7, replied: 5 },
];

const FORWARD = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-00000000fd01",
  kind: "forward" as const,
  conversation_id: MARTA.id,
  message_id: MARTA_DETAIL.messages.at(-1)?.id ?? null,
};

const MARTA_ITEM = ITEMS[1]?.id ?? "";

function api(conversations: Summary[], detail = MARTA_DETAIL) {
  return mockApi({
    [`GET ${W}/attention`]: (request) =>
      new URL(request.url).searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: ITEMS, counts: COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations, next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: detail },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: { body: { ...PHISH, need: null } },
    [`POST ${W}/conversations/${PHISH.id}/need/reopen`]: { body: PHISH },
    [`POST ${W}/attention/${MARTA_ITEM}/snooze`]: { body: ITEMS[1] },
    // Found by name: the server matches what is typed.
    [`GET ${W}/senders`]: (request) => {
      const q = (new URL(request.url).searchParams.get("q") ?? "").toLowerCase();
      return {
        body: {
          senders: PEOPLE.filter((p) =>
            `${p.name ?? ""} ${p.address}`
              .normalize("NFD")
              .replaceAll(/\p{Diacritic}/gu, "")
              .toLowerCase()
              .includes(q),
          ),
        },
      };
    },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`POST ${W}/drafts`]: { status: 201, body: FORWARD },
  });
}

/** The selected row's actions. */
async function actions(name: string) {
  return within(await screen.findByRole("group", { name: `Actions for ${name}` }));
}

const others = [...NEEDS.filter((n) => n.id !== PHISH.id && n.id !== MARTA.id), SOFIA];

describe("What needs you: the actions Scout picked", () => {
  it("offers a need's ranked actions in the server's order, marked with the visor", async () => {
    api([
      ranked(PHISH, [
        { actions: ["safe"], likelihood: 81 },
        { actions: ["report"], likelihood: 19 },
      ]),
      MARTA,
      ...others,
    ]);
    renderHome();
    const row = await actions(PHISH_NAME);
    const own = row.getAllByRole("button").map((b) => b.textContent);
    // It's safe first, as ranked (the fixed order is Report first).
    expect(own[0]).toMatch(/^It’s safe/u);
    expect(own[1]).toMatch(/^Report and delete/u);
    expect(row.getByRole("img", { name: PICKED })).toBeInTheDocument();
    // No percentage anywhere.
    expect(document.body).not.toHaveTextContent("%");
  });

  it("offers exactly the fixed actions when the server sent none", async () => {
    api([...NEEDS, SOFIA]);
    renderHome();
    const row = await actions(PHISH_NAME);
    const own = row.getAllByRole("button").map((b) => b.textContent);
    expect(own[0]).toMatch(/^Report and delete/u);
    expect(own[1]).toMatch(/^It’s safe/u);
    expect(row.queryByRole("img", { name: PICKED })).toBeNull();
  });

  it("takes keys 1 to 3 in the ranked order", async () => {
    const calls = api([
      ranked(PHISH, [
        { actions: ["safe"], likelihood: 81 },
        { actions: ["report"], likelihood: 19 },
      ]),
      MARTA,
      ...others,
    ]);
    renderHome();
    await actions(PHISH_NAME);
    // 1 is now It's safe: the warning is marked safe, not reported.
    fireEvent.keyDown(document, { key: "1" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route.endsWith("/need/resolve"))?.body).toEqual({
        outcome: "not_needed",
        message_id: PHISH.need?.message_id,
      });
    });
    expect(calls.calls.some((c) => c.route.endsWith("/spam"))).toBe(false);
  });

  it("offers a pair done together as one button", async () => {
    api([
      PHISH,
      ranked(MARTA, [
        { actions: ["reply"], likelihood: 90 },
        { actions: ["open_attachment", "forward"], likelihood: 78 },
      ]),
      ...others,
    ]);
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    const row = await actions("Marta Ferreira");
    expect(row.getByRole("button", { name: /^Open attachment, then Forward…/u })).toBeVisible();
    expect(row.queryByRole("button", { name: /^Open attachment$/u })).toBeNull();
    // Pressed, it runs both: Forward asks to whom.
    fireEvent.keyDown(document, { key: "2" });
    expect(
      await screen.findByRole("dialog", { name: "Forward Marta Ferreira’s email to…" }),
    ).toBeVisible();
  });

  it("asks when with Remind, then snoozes it with Undo", async () => {
    const calls = api([
      PHISH,
      ranked(MARTA, [
        { actions: ["reply"], likelihood: 90 },
        { actions: ["remind"], likelihood: 82 },
      ]),
      ...others,
    ]);
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    const row = await actions("Marta Ferreira");
    fireEvent.click(row.getByRole("button", { name: /^Remind me…/u }));
    const dialog = await screen.findByRole("dialog", { name: "Remind me about Marta Ferreira…" });
    fireEvent.click(within(dialog).getByRole("button", { name: /Tomorrow morning/u }));
    await waitFor(() => {
      expect(calls.calls.map((c) => c.route)).toContain(`POST ${W}/attention/${MARTA_ITEM}/snooze`);
    });
    expect((await screen.findAllByRole("button", { name: /^Undo/u })).length).toBeGreaterThan(0);
  });

  it("forwards to someone found by name, then opens the draft in the reader", async () => {
    const calls = api([
      PHISH,
      ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]),
      ...others,
    ]);
    const { router } = renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    // Typed by name, accents or not: only João is left.
    fireEvent.change(within(dialog).getByRole("searchbox", { name: "Find someone by name" }), {
      target: { value: "joao" },
    });
    await waitFor(() => {
      expect(within(dialog).queryByRole("button", { name: /Rita Gomes/u })).toBeNull();
    });
    fireEvent.click(await within(dialog).findByRole("button", { name: /João Matos/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toContain(MARTA.id);
    });
    const created = calls.calls.find((c) => c.route === `POST ${W}/drafts`)?.body as {
      kind: string;
      to: { address: string }[];
      quoted: string;
    };
    expect(created.kind).toBe("forward");
    expect(created.to).toEqual([{ name: "João Matos", address: "joao@obras-matos.example" }]);
    expect(created.quoted).toContain("Forwarded message");
  });

  it("opens the item on its file with Open attachment", async () => {
    const message = MARTA_DETAIL.messages[0];
    const withFile = {
      ...MARTA_DETAIL,
      messages:
        message === undefined
          ? []
          : [
              {
                ...message,
                attachments: [
                  {
                    id: "0192f1a0-0000-7000-8000-0000000000a1",
                    filename: "walkthrough-plan.pdf",
                    content_type: "application/pdf",
                    size: 48_000,
                    inline: false,
                    in_body: false,
                  },
                ],
              },
            ],
    };
    api(
      [PHISH, ranked(MARTA, [{ actions: ["open_attachment"], likelihood: 77 }]), ...others],
      withFile,
    );
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    await waitFor(() => {
      expect(document.activeElement?.textContent).toContain("walkthrough-plan.pdf");
    });
  });

  it("picks the first match with Enter in the search box", async () => {
    const calls = api([
      PHISH,
      ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]),
      ...others,
    ]);
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    const box = within(dialog).getByRole("searchbox", { name: "Find someone by name" });
    fireEvent.change(box, { target: { value: "rita" } });
    await within(dialog).findByRole("button", { name: /Rita Gomes/u });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route === `POST ${W}/drafts`)?.body).toMatchObject({
        kind: "forward",
        to: [{ name: "Rita Gomes", address: "rita@silva-contas.example" }],
      });
    });
  });
});
