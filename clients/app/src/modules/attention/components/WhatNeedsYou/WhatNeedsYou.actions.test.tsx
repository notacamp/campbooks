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
  ACME,
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
import { INBOX, NOW, W } from "../../../../../test/demo/conversations";
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

/** Senders the server leaves out with `reachable` (campbooks-cloud#233). */
const UNSAFE: Schema<"SenderView">[] = [
  { address: "it-support@at3lier-sol.example", name: "IT Support", received: 1, replied: 0 },
  { address: "no-reply@accounts.example", name: "Accounts", received: 4, replied: 0 },
];

const FORWARD = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-00000000fd01",
  kind: "forward" as const,
  conversation_id: MARTA.id,
  message_id: MARTA_DETAIL.messages.at(-1)?.id ?? null,
};

const MARTA_ITEM = ITEMS[1]?.id ?? "";

/** Marta's email with a file of its own. */
const WITH_FILE: Schema<"ConversationDetail"> = {
  ...MARTA_DETAIL,
  messages: MARTA_DETAIL.messages.map((message) => ({
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
  })),
};

interface Options {
  /** The member's role in Sam's inbox. */
  role?: "manager" | "reader";
  /** Marta's drafts already there. */
  drafts?: Schema<"DraftView">[];
}

function api(conversations: Summary[], detail = MARTA_DETAIL, options: Options = {}) {
  // The drafts started here, as the server would list them afterwards.
  let started: Schema<"DraftView">[] = [];
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role: options.role ?? "manager" }] } },
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
    // Without `reachable`, the server would list the unsafe senders too.
    [`GET ${W}/senders`]: (request) => {
      const params = new URL(request.url).searchParams;
      const q = (params.get("q") ?? "").toLowerCase();
      const known = params.get("reachable") === "true" ? PEOPLE : [...PEOPLE, ...UNSAFE];
      return {
        body: {
          senders: known.filter((p) =>
            `${p.name ?? ""} ${p.address}`
              .normalize("NFD")
              .replaceAll(/\p{Diacritic}/gu, "")
              .toLowerCase()
              .includes(q),
          ),
        },
      };
    },
    [`GET ${W}/drafts`]: () => ({ body: { drafts: [...started, ...(options.drafts ?? [])] } }),
    [`POST ${W}/drafts`]: () => {
      started = [FORWARD];
      return { status: 201, body: FORWARD };
    },
    [`PATCH ${W}/drafts/${FORWARD.id}`]: { body: FORWARD },
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
    api(
      [PHISH, ranked(MARTA, [{ actions: ["open_attachment"], likelihood: 77 }]), ...others],
      WITH_FILE,
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

  it("keeps the focus in Forward's dialog when a pair opens the file first", async () => {
    api(
      [
        PHISH,
        ranked(MARTA, [{ actions: ["open_attachment", "forward"], likelihood: 80 }]),
        ...others,
      ],
      WITH_FILE,
    );
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    // The item opened on its email, behind the dialog…
    expect(await screen.findByText("walkthrough-plan.pdf")).toBeInTheDocument();
    await new Promise((resolve) => setTimeout(resolve, 50));
    // …and the focus stayed in the dialog's search box.
    expect(within(dialog).getByRole("searchbox", { name: "Find someone by name" })).toHaveFocus();
  });

  it("offers a read-only inbox only what doesn't write there", async () => {
    api(
      [
        PHISH,
        ranked(MARTA, [
          { actions: ["reply"], likelihood: 92 },
          { actions: ["forward"], likelihood: 86 },
          { actions: ["remind"], likelihood: 82 },
        ]),
        ...others,
      ],
      MARTA_DETAIL,
      { role: "reader" },
    );
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    const row = await actions("Marta Ferreira");
    await waitFor(() => {
      expect(row.queryByRole("button", { name: /^Reply/u })).toBeNull();
    });
    expect(row.queryByRole("button", { name: /^Forward…/u })).toBeNull();
    expect(row.getByRole("button", { name: /^Remind me…/u })).toBeVisible();
    expect(row.getByRole("button", { name: /^Snooze…/u })).toBeVisible();
    expect(row.queryByRole("button", { name: /^Done/u })).toBeNull();
  });

  it("offers a bill's reminder before its due date first in Remind", async () => {
    api([
      PHISH,
      MARTA,
      ...others.map((summary) =>
        summary.id === ACME.id
          ? ranked(summary, [
              { actions: ["already_paid"], likelihood: 40 },
              { actions: ["still_to_pay"], likelihood: 60 },
              { actions: ["remind"], likelihood: 85 },
            ])
          : summary,
      ),
    ]);
    renderHome();
    await actions(PHISH_NAME);
    for (let step = 0; step < 5; step += 1) {
      fireEvent.keyDown(document, { key: "j" });
    }
    const row = await actions("Acme Hosting");
    fireEvent.click(row.getByRole("button", { name: /^Remind me…/u }));
    const dialog = await screen.findByRole("dialog", { name: "Remind me about Acme Hosting…" });
    // The first choice (its number key is 1).
    const first = within(dialog)
      .getAllByRole("button")
      .find((button) => button.dataset["dialogChoice"] !== undefined);
    expect(first).toHaveTextContent(/Before it’s due/u);
  });

  it("addresses an untouched forward again, and starts a new one beside a written one", async () => {
    const untouched = { ...FORWARD, to: [] };
    const calls = api(
      [PHISH, ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]), ...others],
      MARTA_DETAIL,
      { drafts: [untouched] },
    );
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(dialog).findByRole("button", { name: /Rita Gomes/u }));
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route === `PATCH ${W}/drafts/${FORWARD.id}`)?.body).toEqual({
        to: [{ name: "Rita Gomes", address: "rita@silva-contas.example" }],
        version: FORWARD.version,
      });
    });
    expect(calls.calls.some((c) => c.route === `POST ${W}/drafts`)).toBe(false);
  });

  it("never re-addresses a forward the member wrote in", async () => {
    const written = {
      ...FORWARD,
      to: [{ name: "Ana", address: "ana@example.com" }],
      body: "For Ana",
    };
    const calls = api(
      [PHISH, ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]), ...others],
      MARTA_DETAIL,
      { drafts: [written] },
    );
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(dialog).findByRole("button", { name: /Rita Gomes/u }));
    await waitFor(() => {
      expect(calls.calls.some((c) => c.route === `POST ${W}/drafts`)).toBe(true);
    });
    expect(calls.calls.some((c) => c.route.startsWith("PATCH"))).toBe(false);
  });

  it("goes nowhere when Forward is closed while it starts", async () => {
    api([PHISH, ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]), ...others]);
    const { router } = renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(dialog).findByRole("button", { name: /Rita Gomes/u }));
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(router.state.location.pathname).not.toContain(MARTA.id);
  });

  it("never offers to forward to a sender under review or a no-reply address", async () => {
    api([PHISH, ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]), ...others]);
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    await within(dialog).findByRole("button", { name: /Rita Gomes/u });
    expect(within(dialog).queryByRole("button", { name: /IT Support/u })).toBeNull();
    expect(within(dialog).queryByRole("button", { name: /Accounts/u })).toBeNull();
    // Typed by name, they find nobody: no warning, no list.
    const box = within(dialog).getByRole("searchbox", { name: "Find someone by name" });
    for (const typed of ["support", "accounts"]) {
      fireEvent.change(box, { target: { value: typed } });
      expect(
        await within(dialog).findByText("No one by that name has written to you."),
      ).toBeVisible();
      expect(within(dialog).queryByRole("button", { name: /IT Support|Accounts/u })).toBeNull();
    }
  });

  it("addresses the same draft when Forward is reopened after Esc mid-start", async () => {
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
    const first = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(first).findByRole("button", { name: /Rita Gomes/u }));
    fireEvent.keyDown(first, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await waitFor(() => {
      expect(calls.calls.filter((c) => c.route === `POST ${W}/drafts`)).toHaveLength(1);
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    fireEvent.keyDown(document, { key: "1" });
    const again = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(again).findByRole("button", { name: /João Matos/u }));
    await waitFor(() => {
      expect(calls.calls.find((c) => c.route === `PATCH ${W}/drafts/${FORWARD.id}`)?.body).toEqual({
        to: [{ name: "João Matos", address: "joao@obras-matos.example" }],
        version: FORWARD.version,
      });
    });
    expect(calls.calls.filter((c) => c.route === `POST ${W}/drafts`)).toHaveLength(1);
  });

  it("opens nothing when the drafts can't be read while a forward starts", async () => {
    api([PHISH, ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]), ...others]);
    // The draft's creation waits; meanwhile the drafts can no longer be read.
    const mocked = globalThis.fetch;
    let release!: () => void;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    let unreadable = false;
    globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
      const request = input instanceof Request ? input : new Request(input, init);
      const path = new URL(request.url).pathname;
      if (path === `${W}/drafts` && request.method === "POST") {
        await held;
      } else if (path === `${W}/drafts` && unreadable) {
        return Response.json({ title: "Unavailable", status: 503 }, { status: 503 });
      }
      return mocked(request);
    };
    const { router, queryClient } = renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(dialog).findByRole("button", { name: /Rita Gomes/u }));
    await within(dialog).findByText("Starting the forward…");
    unreadable = true;
    await queryClient.refetchQueries({
      predicate: (query) => JSON.stringify(query.queryKey).includes("conversations.list_drafts"),
    });
    expect(
      await within(dialog).findByText("Couldn’t start the forward: nothing was sent. Try again."),
    ).toBeVisible();
    release();
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(router.state.location.pathname).not.toContain(MARTA.id);
    expect(
      screen.getByRole("dialog", { name: "Forward Marta Ferreira’s email to…" }),
    ).toBeVisible();
  });

  it("says so when Open attachment finds no file to open", async () => {
    api([PHISH, ranked(MARTA, [{ actions: ["open_attachment"], likelihood: 77 }]), ...others]);
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    expect(await screen.findByText("This email has no file to open.")).toBeInTheDocument();
  });

  it("is never stuck starting when the email can't be read", async () => {
    // Marta's conversation has no message the need names.
    api([PHISH, ranked(MARTA, [{ actions: ["forward"], likelihood: 88 }]), ...others], {
      ...MARTA_DETAIL,
      messages: [],
    });
    renderHome();
    await actions(PHISH_NAME);
    fireEvent.keyDown(document, { key: "j" });
    await actions("Marta Ferreira");
    fireEvent.keyDown(document, { key: "1" });
    const dialog = await screen.findByRole("dialog", {
      name: "Forward Marta Ferreira’s email to…",
    });
    fireEvent.click(await within(dialog).findByRole("button", { name: /Rita Gomes/u }));
    expect(
      await within(dialog).findByText("Couldn’t start the forward: nothing was sent. Try again."),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText("Starting the forward…")).toBeNull();
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
