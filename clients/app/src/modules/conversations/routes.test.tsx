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
  INBOX,
  LEDGER,
  LEDGER_DETAIL,
  LEDGER_RULE,
  LEDGER_RUN,
  LEDGER_SUGGESTIONS,
  ledgerBody,
  NDA,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  SENT,
  STREAM_COUNTS,
  W,
} from "../../../test/demo/conversations";
import { BLANK, DRAFT_LIST, REPLY_WRITTEN, TO_MARTA } from "../../../test/demo/drafts";
import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";
import type { Schema } from "~/lib/api";

import { buildConversationsRoutes } from "./index";

function renderAt(path: string, locale: "en" | "pt-PT" = "en") {
  const root = createRootRoute({ component: Outlet });
  const workspace = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: Outlet,
  });
  const router = createRouter({
    routeTree: root.addChildren([workspace.addChildren(buildConversationsRoutes(workspace))]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  return { router, ...renderWithProviders(<RouterProvider router={router} />, locale) };
}

/** A fixture list's first item. */
function firstOf<T>(items: readonly T[]): T {
  const [item] = items;
  if (item === undefined) {
    throw new Error("the fixture is empty");
  }
  return item;
}

/** A finished apply run, to vary. */
const RUN = LEDGER_RUN;

const summary = (id: string) => PRIMARY.find((conversation) => conversation.id === id);

function inbox(extra: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/conversations`]: (request) => {
      const query = new URL(request.url).searchParams;
      const stream = query.get("stream");
      const view = query.get("view");
      return {
        body: {
          conversations: stream === "primary" ? PRIMARY : view === "sent" ? SENT : [],
          next_cursor: null,
        },
      };
    },
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
    [`GET ${W}/conversations/${LEDGER.id}`]: { body: LEDGER_DETAIL },
    [`GET ${W}/conversations/${Q4.id}/messages/${Q4_DETAIL.messages[2]?.id ?? ""}/body`]: {
      body: Q4_BODY,
    },
    [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]: (
      request,
    ) => ({ body: ledgerBody(new URL(request.url).searchParams.get("show_images") === "true") }),
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`POST ${W}/conversations/${LEDGER.id}/read`]: { body: { ...LEDGER, unread_count: 0 } },
    [`POST ${W}/conversations/${Q4.id}/archive`]: { body: { ...Q4, archived: true } },
    [`POST ${W}/conversations/${Q4.id}/unarchive`]: { body: Q4 },
    [`POST ${W}/conversations/${Q4.id}/trash`]: { body: { ...Q4, trashed: true } },
    [`POST ${W}/conversations/${Q4.id}/untrash`]: { body: Q4 },
    [`POST ${W}/conversations/${Q4.id}/spam`]: {
      body: { ...Q4, archived: true, trashed: true, spam: true },
    },
    [`POST ${W}/conversations/${Q4.id}/not-spam`]: { body: Q4 },
    [`POST ${W}/conversations/${LEDGER.id}/stream`]: (_request, body) => ({
      body: { ...LEDGER, ...(body as object), stream_source: "member", suggestion: null },
    }),
    // The smart-rule confirmation counts what the rule matches.
    [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: {
      body: LEDGER_SUGGESTIONS.suggestions[0]?.preview,
    },
    ...extra,
  });
}

describe("the inbox", () => {
  it("lists a stream newest first, with its suggestion shown as a suggestion", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary`);

    expect(await screen.findByRole("heading", { level: 1, name: "Primary" })).toBeVisible();
    const list = screen.getByRole("region", { name: "Conversations" });
    expect(await within(list).findByText(Q4.subject)).toBeVisible();
    expect(within(list).getByText("Suggests Newsletters")).toBeVisible();
    expect(screen.getByRole("heading", { name: "Pick a conversation" })).toBeVisible();
  });

  it("goes to Primary from an unknown stream or view", async () => {
    inbox();
    const { router } = renderAt(`/w/${DUARTE.id}/inbox/promotions`);

    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/primary`);
    });
  });

  it("lists Sent by your latest message: to whom, and whether they replied", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/sent`);

    expect(await screen.findByRole("heading", { level: 1, name: "Sent" })).toBeVisible();
    const list = screen.getByRole("region", { name: "Conversations" });
    expect(await within(list).findByText("To Marta Ferreira and 2 others")).toBeVisible();
    expect(within(list).getByText("To Maya Okafor")).toBeVisible();
    expect(within(list).getAllByText("Replied")).toHaveLength(1);
  });

  it("says when a view is empty, in one plain line", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/spam`);

    expect(await screen.findByRole("heading", { level: 1, name: "Spam" })).toBeVisible();
    expect(await screen.findByRole("heading", { name: "Nothing in Spam." })).toBeVisible();
  });

  it("lists what you pinned in Pinned, archived or not", async () => {
    inbox({
      [`GET ${W}/conversations`]: (request) => ({
        body: {
          conversations: new URL(request.url).searchParams.get("view") === "starred" ? [NDA] : [],
          next_cursor: null,
        },
      }),
    });
    renderAt(`/w/${DUARTE.id}/inbox/starred`);

    expect(await screen.findByRole("heading", { level: 1, name: "Pinned" })).toBeVisible();
    const list = screen.getByRole("region", { name: "Conversations" });
    expect(await within(list).findByText(NDA.subject)).toBeVisible();
  });

  it("lists your drafts, and opens a new message where it was left", async () => {
    inbox({ [`GET ${W}/drafts`]: { body: { drafts: DRAFT_LIST } } });
    const { router } = renderAt(`/w/${DUARTE.id}/inbox/drafts`);

    expect(await screen.findByRole("heading", { level: 1, name: "Drafts" })).toBeVisible();
    expect(await screen.findByText("Draft · Marta Ferreira and 2 others")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Draft · Marta Ferreira/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/compose`);
    });
    expect(router.state.location.search).toEqual({ draft: TO_MARTA.id });
  });

  it("opens a reply draft in its thread, beside the Drafts list", async () => {
    inbox({ [`GET ${W}/drafts`]: { body: { drafts: [REPLY_WRITTEN] } } });
    const { router } = renderAt(`/w/${DUARTE.id}/inbox/drafts`);

    fireEvent.click(await screen.findByRole("button", { name: /Draft · Maya Okafor/u }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/inbox/drafts/${Q4.id}`);
    });
    expect(router.state.location.search).toEqual({ draft: REPLY_WRITTEN.id });
    expect(await screen.findByDisplayValue(/^Thanks, Maya\./u)).toBeVisible();
    expect(screen.getByRole("heading", { level: 1, name: "Drafts" })).toBeVisible();
  });

  it("opens the draft of the row pressed when a conversation holds several", async () => {
    const forward = {
      ...REPLY_WRITTEN,
      id: "0192f1a0-0000-7000-8000-000000000105",
      kind: "forward" as const,
      to: [],
      subject: `Fwd: ${Q4.subject}`,
      body: "Forwarding this to the team.",
      updated_at: "2026-10-13T14:10:00Z",
    };
    inbox({ [`GET ${W}/drafts`]: { body: { drafts: [REPLY_WRITTEN, forward] } } });
    const { router } = renderAt(`/w/${DUARTE.id}/inbox/drafts`);

    fireEvent.click(await screen.findByRole("button", { name: /Draft · No recipients/u }));
    expect(await screen.findByDisplayValue("Forwarding this to the team.")).toBeVisible();
    expect(screen.queryByDisplayValue(/^Thanks, Maya\./u)).toBeNull();
    expect(router.state.location.search).toEqual({ draft: forward.id });
  });

  it("deletes a draft from the list, and Undo brings it back", async () => {
    let stored = [TO_MARTA, BLANK];
    const api = inbox({
      [`GET ${W}/drafts`]: () => ({ body: { drafts: stored } }),
      [`DELETE ${W}/drafts/${TO_MARTA.id}`]: () => {
        stored = stored.filter((draft) => draft.id !== TO_MARTA.id);
        return { status: 204 };
      },
      [`POST ${W}/drafts`]: () => {
        stored = [TO_MARTA, BLANK];
        return { status: 201, body: TO_MARTA };
      },
    });
    renderAt(`/w/${DUARTE.id}/inbox/drafts`);

    await screen.findByText("Figures for the board");
    fireEvent.click(screen.getByRole("button", { name: "Delete draft: Figures for the board" }));
    expect(await screen.findByText("Draft deleted")).toBeVisible();
    await waitFor(() => {
      expect(screen.queryByText("Figures for the board")).toBeNull();
    });
    fireEvent.click(screen.getByRole("button", { name: /^Undo/u }));
    expect(await screen.findByText("Figures for the board")).toBeVisible();
    expect(api.calls.some((call) => call.route === `POST ${W}/drafts`)).toBe(true);
  });

  it("deletes with # and takes it back with Z", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);
    await screen.findByRole("heading", { name: Q4.subject });

    fireEvent.keyDown(document, { key: "#" });
    expect(await screen.findByText(`Deleted · ${Q4.subject}`)).toBeVisible();
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/trash`,
      );
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/untrash`,
      );
    });
  });

  it("undoes Report spam on an archived conversation back into the archive, in one Undo", async () => {
    const archived = { ...Q4, archived: true, unread_count: 0 };
    const api = inbox({
      [`GET ${W}/conversations/${Q4.id}`]: { body: { ...Q4_DETAIL, conversation: archived } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/archive/${Q4.id}`);
    await screen.findByRole("heading", { name: Q4.subject });

    fireEvent.keyDown(document, { key: "!" });
    expect(await screen.findByText(`Reported as spam · ${Q4.subject}`)).toBeVisible();
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/archive`,
      );
    });
    const moves = api.calls
      .map((call) => call.route)
      .filter((route) => /\/(spam|not-spam|archive)$/u.test(route));
    expect(moves).toEqual([
      `POST ${W}/conversations/${Q4.id}/spam`,
      `POST ${W}/conversations/${Q4.id}/not-spam`,
      `POST ${W}/conversations/${Q4.id}/archive`,
    ]);
  });

  it("offers no Delete in spam: # does nothing there, ! is the way out", async () => {
    const spammed = { ...Q4, archived: true, trashed: true, spam: true, unread_count: 0 };
    const api = inbox({
      [`GET ${W}/conversations/${Q4.id}`]: { body: { ...Q4_DETAIL, conversation: spammed } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/spam/${Q4.id}`);
    await screen.findByRole("heading", { name: Q4.subject });
    expect(screen.queryByRole("button", { name: /^Delete/u })).toBeNull();

    // Deleting would clear the spam mark, and its Undo would not bring it back.
    fireEvent.keyDown(document, { key: "#" });
    fireEvent.keyDown(document, { key: "!" });
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/not-spam`,
      );
    });
    expect(api.calls.map((call) => call.route)).not.toContain(
      `POST ${W}/conversations/${Q4.id}/trash`,
    );
  });

  it("opens a conversation: newest message open, older folded, read on open", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(await within(reader).findByRole("heading", { name: Q4.subject })).toBeVisible();
    // The newest message is open, in the sandboxed frame.
    const frame = await within(reader).findByTitle("Message from Maya Okafor");
    expect(frame.getAttribute("sandbox")).toBe(
      "allow-same-origin allow-popups allow-popups-to-escape-sandbox",
    );
    expect(within(reader).getByText("q4-plan-v3.pdf")).toBeVisible();
    // The two older ones are folded until opened.
    const folded = within(reader).getAllByRole("button", { expanded: false });
    expect(folded.length).toBeGreaterThanOrEqual(2);
    fireEvent.click(within(reader).getByRole("button", { name: /^You\s*→ Maya Okafor/u }));
    expect(await within(reader).findByText(/Is the timeline fixed\?/u)).toBeVisible();
    // The note says what the decide pass thinks, as suggestions; how it
    // decided (the only place with numbers) is folded away.
    const note = within(reader).getByRole("region", { name: "Where things stand" });
    expect(note).toHaveTextContent("it needs a reply and it mentions a date to meet");
    expect(note).not.toHaveTextContent("%");
    fireEvent.click(within(note).getByText("How Campbooks decided"));
    expect(within(note).getByText("Yes. It names a date (“Friday”).")).toBeVisible();
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/read`,
      );
    });
  });

  it("folds a message that came without its words with them as its snippet", async () => {
    const first = Q4_DETAIL.messages[0]?.id ?? "";
    let answer!: () => void;
    const held = new Promise<void>((resolve) => {
      answer = resolve;
    });
    const api = inbox({
      [`GET ${W}/conversations/${Q4.id}`]: {
        body: {
          ...Q4_DETAIL,
          messages: Q4_DETAIL.messages.map((message) =>
            message.id === first ? { ...message, text: null, text_unread: true } : message,
          ),
        },
      },
      [`GET ${W}/conversations/${Q4.id}/messages/${first}/body`]: {
        body: {
          ...Q4_BODY,
          text: "The harbourlight pontoon plan, read from its HTML.",
          html: null,
        },
      },
    });
    const mocked = globalThis.fetch;
    globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
      const request = input instanceof Request ? input : new Request(input, init);
      if (new URL(request.url).pathname.endsWith(`/messages/${first}/body`)) {
        await held;
      }
      return mocked(request);
    };
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    const reader = await screen.findByRole("main", { name: "Conversation" });
    // A neutral word while it reads, never an empty or made-up snippet.
    expect(await within(reader).findByText("Reading…")).toBeVisible();
    answer();
    expect(
      await within(reader).findByText("The harbourlight pontoon plan, read from its HTML."),
    ).toBeVisible();
    const asked = api.calls.filter((call) => call.route.endsWith(`/messages/${first}/body`));
    expect(asked).toHaveLength(1);
  });

  it("shows a kept inbox's mail as read-only history, with nothing to act on", async () => {
    const api = inbox({
      [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, read_only: true }] } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(await within(reader).findByText("Kept as history")).toBeVisible();
    for (const name of [/^Done/u, /^Reply/u, /^Pin/u]) {
      expect(within(reader).queryByRole("button", { name })).toBeNull();
    }
    fireEvent.keyDown(document, { key: "e" });
    expect(api.calls.map((call) => call.route)).not.toContain(
      `POST ${W}/conversations/${Q4.id}/read`,
    );
    expect(api.calls.map((call) => call.route)).not.toContain(
      `POST ${W}/conversations/${Q4.id}/archive`,
    );
  });

  it("offers nothing to act on until it knows the inbox can be written", async () => {
    const api = inbox({ [`GET ${W}/inboxes`]: { status: 503 } });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(await within(reader).findByRole("heading", { name: Q4.subject })).toBeVisible();
    expect(within(reader).queryByRole("button", { name: /^Done/u })).toBeNull();
    expect(api.calls.map((call) => call.route)).not.toContain(
      `POST ${W}/conversations/${Q4.id}/read`,
    );
  });

  it("archives with E and takes it back with Z", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);
    await screen.findByRole("heading", { name: Q4.subject });

    fireEvent.keyDown(document, { key: "e" });
    expect(await screen.findByText(`Done · ${Q4.subject}`)).toBeVisible();
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/archive`,
      );
    });
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toContain(
        `POST ${W}/conversations/${Q4.id}/unarchive`,
      );
    });
  });

  it("takes a suggestion with 1, and shows images only when asked", async () => {
    const api = inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });

    const frame = await within(reader).findByTitle("Message from The Weekly Ledger");
    expect(frame.getAttribute("srcdoc")).toContain("img-src data:;");
    expect(within(reader).getByText(/1 remote image hidden/u)).toBeVisible();
    fireEvent.click(within(reader).getByRole("button", { name: /Show images/u }));
    await waitFor(() => {
      expect(
        within(reader).getByTitle("Message from The Weekly Ledger").getAttribute("srcdoc"),
      ).toContain("img-src data: https: http:");
    });

    fireEvent.keyDown(document, { key: "1" });
    expect(await screen.findByText(`Moved to Newsletters · ${LEDGER.subject}`)).toBeVisible();
    const moved = api.calls.find(
      (call) => call.route === `POST ${W}/conversations/${LEDGER.id}/stream`,
    );
    expect(moved?.body).toEqual({ stream: "newsletters" });
  });

  it("always shows a trusted sender's images, once the smart rule is confirmed", async () => {
    // The server trusts the sender once the rule is saved.
    let trusted = false;
    const rule = {
      ...LEDGER_RULE,
      conditions: [{ field: "sender_domain" as const, value: "weekly-ledger.example" }],
      actions: [{ action: "show_images" as const }],
      applied_count: 0,
    };
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]:
        (request) => ({
          body: {
            ...ledgerBody(
              trusted || new URL(request.url).searchParams.get("show_images") === "true",
            ),
            images_trusted: trusted,
            ...(trusted ? { images_offer: false, images_domain: null } : {}),
          },
        }),
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: {
        body: {
          matching: 3,
          would_change: 0,
          samples: [],
          skipped_suspicious: 0,
          skipped_member: 0,
          overrides: [],
          complete: true,
        },
      },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: () => {
        trusted = true;
        return { status: 201, body: { rule, acted_without_asking: false } };
      },
      [`GET ${W}/rules`]: { body: { rules: [rule] } },
      [`GET ${W}/rules/${rule.id}`]: { body: rule },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByText(/1 remote image hidden/u);

    fireEvent.click(within(reader).getByRole("button", { name: /^Always from this sender/u }));
    const dialog = await screen.findByRole("dialog", {
      name: "Always show images from hello@weekly-ledger.example",
    });
    // The address by default, or anyone at its domain; a rule with no AI.
    expect(
      within(dialog).getByRole("radio", { name: /Only hello@weekly-ledger.example/u }),
    ).toBeChecked();
    expect(
      await within(dialog).findByText(
        "3 conversations from them are already here. Their images show whenever their mail passes its checks.",
      ),
    ).toBeVisible();
    expect(within(dialog).getByText(/Tracking pixels stay removed/u)).toBeVisible();
    fireEvent.click(
      within(dialog).getByRole("radio", { name: /Anyone at weekly-ledger.example/u }),
    );
    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));

    await waitFor(() => {
      expect(
        api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`)?.body,
      ).toMatchObject({
        conditions: [{ field: "sender_domain", value: "weekly-ledger.example" }],
        actions: [{ action: "show_images" }],
        apply_to_existing: false,
        from_conversation_id: LEDGER.id,
      });
    });
    // Its images show without asking, and the reader says why.
    expect(
      await within(reader).findByText(
        "Images shown: you trust The Weekly Ledger. Tracking pixels stay removed.",
      ),
    ).toBeVisible();
    expect(within(reader).queryByRole("button", { name: /^Show images/u })).toBeNull();
    // Undo takes the rule back at once.
    expect((await screen.findAllByRole("button", { name: /^Undo/u })).length).toBeGreaterThan(0);
  });

  it("opens the trust confirmation with Shift+A", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByText(/1 remote image hidden/u);
    fireEvent.keyDown(document, { key: "A", shiftKey: true });
    expect(
      await screen.findByRole("dialog", {
        name: "Always show images from hello@weekly-ledger.example",
      }),
    ).toBeVisible();
  });

  it("says why images stay held back where trusting the sender could not take effect", async () => {
    inbox({
      [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]:
        {
          body: {
            ...ledgerBody(false),
            images_offer: false,
            images_domain: null,
            images_held: "checks_fail",
          },
        },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(
      await within(reader).findByText(
        "Images stay held back: this email doesn’t pass its sender’s checks.",
      ),
    ).toBeVisible();
    expect(within(reader).getByRole("button", { name: /^Show images/u })).toBeVisible();
    expect(within(reader).queryByRole("button", { name: /^Always show images/u })).toBeNull();
    // ⇧A trusts nobody here.
    fireEvent.keyDown(document, { key: "A", shiftKey: true });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("offers the address alone when the server offers no domain", async () => {
    inbox({
      [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]:
        { body: { ...ledgerBody(false), images_domain: null } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    fireEvent.click(
      await within(reader).findByRole("button", {
        name: /^Always from this sender/u,
      }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Always show images from hello@weekly-ledger.example",
    });
    expect(within(dialog).queryByRole("radio")).toBeNull();
    // The one choice still says what the checks vouch for (campbooks-cloud#236).
    expect(
      within(dialog).getByText(
        "Mail from this address when its domain’s checks pass. Those checks vouch for the domain, not for this one person.",
      ),
    ).toBeVisible();
  });

  it("says the inbox trusts the sender when others use it", async () => {
    inbox({
      [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, shared: true }] } },
      [`GET ${W}/conversations/${LEDGER.id}/messages/${LEDGER_DETAIL.messages[0]?.id ?? ""}/body`]:
        {
          body: {
            ...ledgerBody(true),
            images_trusted: true,
            images_offer: false,
            images_domain: null,
          },
        },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    expect(
      await within(reader).findByText(
        "Images shown: this inbox trusts The Weekly Ledger. Tracking pixels stay removed.",
      ),
    ).toBeVisible();
  });

  it("offers no trust in an inbox the member only reads", async () => {
    inbox({ [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role: "reader" }] } } });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByText(/1 remote image hidden/u);
    expect(within(reader).getByRole("button", { name: /^Show images/u })).toBeVisible();
    expect(within(reader).queryByRole("button", { name: /^Always show images/u })).toBeNull();
  });

  it("offers to do the same to every lookalike, then shows the rule's run with Undo all", async () => {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: {
        status: 201,
        body: { rule: LEDGER_RULE, acted_without_asking: false },
      },
      [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: LEDGER_RULE },
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });

    // Move it (the suggestion's 1), and the offer asks about the 40 like it.
    fireEvent.keyDown(document, { key: "1" });
    const offer = await screen.findByRole("dialog", {
      name: "Move all 40 like this to Newsletters, and new ones too?",
    });
    expect(within(offer).getByText("What changes for freelancers in 2027")).toBeVisible();
    expect(within(offer).getByText("None of them looks suspicious.")).toBeVisible();

    // 1 accepts: the smart-rule confirmation shows the rule; nothing is
    // saved before Save smart rule, then the rule is made from the chosen
    // likeness.
    fireEvent.keyDown(document, { key: "1" });
    const confirm = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    expect(within(confirm).getByText("Smart rule")).toBeVisible();
    expect(api.calls.some((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`)).toBe(
      false,
    );
    await within(confirm).findByText("It matches 40 emails already here.");
    fireEvent.click(within(confirm).getByRole("button", { name: /Save smart rule/u }));
    expect(
      await screen.findByRole("dialog", {
        name: "Moved 40 to Newsletters. New ones will go there too.",
      }),
    ).toBeVisible();
    const made = api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`);
    expect(made?.body).toMatchObject({
      conditions: LEDGER_SUGGESTIONS.suggestions[0]?.conditions,
      actions: [{ action: "move_to_stream", stream: "newsletters" }],
      apply_to_existing: true,
      from_conversation_id: LEDGER.id,
    });

    // Undo all puts them back.
    fireEvent.click(screen.getByRole("button", { name: "Undo all" }));
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === `DELETE ${W}/rules/${LEDGER_RULE.id}`)).toBe(
        true,
      );
    });
  });

  it("forgets the offer for this mail with Just this one", async () => {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      [`POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`]: { status: 204 },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    await screen.findByRole("dialog", { name: /Move all 40/u });
    fireEvent.click(screen.getByRole("button", { name: /Just this one/u }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Move all 40/u })).toBeNull();
    });
    const dismissed = api.calls.find(
      (call) => call.route === `POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`,
    );
    expect(dismissed?.body).toEqual({ key: "sender|ledger" });
    // Recorded (204): no word of failure.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByText(/could not record/u)).toBeNull();
  });

  it("dismisses the offer with Esc as soon as it appears, leaving the reader open", async () => {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      [`POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`]: { status: 204 },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    const offer = await screen.findByRole("dialog", { name: /Move all 40/u });
    // The offer took focus: Esc is its own, and its button says so.
    await waitFor(() => {
      expect(offer).toContainElement(document.activeElement as HTMLElement);
    });
    expect(within(offer).getByRole("button", { name: /Just this one/u })).toHaveAttribute(
      "aria-keyshortcuts",
    );
    fireEvent.keyDown(document.activeElement ?? document, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Move all 40/u })).toBeNull();
    });
    expect(screen.getByRole("main", { name: "Conversation" })).toBeVisible();
    expect(api.calls.some((call) => call.route.endsWith("/rule-suggestions/dismiss"))).toBe(true);
  });

  /** Open the Ledger, move it with 1 and accept the offer with 1: the rule's run shows. */
  async function teachLedger(
    rule: () => Schema<"RuleView">,
    extra: Parameters<typeof mockApi>[0] = {},
  ) {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: {
        status: 201,
        body: { rule: rule(), acted_without_asking: false },
      },
      [`GET ${W}/rules/${LEDGER_RULE.id}`]: () => ({ body: rule() }),
      [`GET ${W}/rules`]: () => ({ body: { rules: [rule()] } }),
      ...extra,
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    await screen.findByRole("dialog", { name: /Move all 40/u });
    fireEvent.keyDown(document, { key: "1" });
    const confirm = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    await within(confirm).findByText("It matches 40 emails already here.");
    fireEvent.click(within(confirm).getByRole("button", { name: /Save smart rule/u }));
    // Its result shows for a moment, then the run's progress is the news.
    await waitFor(
      () => {
        expect(screen.queryByText("Smart rule saved")).toBeNull();
      },
      { timeout: 3000 },
    );
    return api;
  }

  const running = (run: Partial<NonNullable<Schema<"RuleView">["run"]>>) => ({
    ...LEDGER_RULE,
    run: { ...RUN, ...run },
  });

  it("counts first, claiming no figure, then says the offer's number while it works", async () => {
    await teachLedger(() => running({ state: "running", counted: false, total: 0, done: 0 }));
    const offer = await screen.findByRole("dialog", { name: "Finding all the mail like this" });
    const bar = within(offer).getByRole("progressbar");
    expect(bar).not.toHaveAttribute("aria-valuenow");
    expect(within(offer).getByText("Counting…")).toBeVisible();
  });

  it("shows the run's progress with the number the offer gave", async () => {
    await teachLedger(() => running({ state: "running", counted: true, total: 41, done: 10 }));
    const offer = await screen.findByRole("dialog", { name: "Moving 40 to Newsletters" });
    expect(within(offer).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "10");
    expect(within(offer).getByText("10 of 41 checked")).toBeVisible();
    expect(within(offer).getByRole("button", { name: "Undo all" })).toBeVisible();
  });

  it("says a cancelled run stopped, and offers no Undo all", async () => {
    await teachLedger(() => running({ state: "cancelled", changed: 12, done: 12 }));
    const offer = await screen.findByRole("dialog", {
      name: "Stopped after changing 12. The rule no longer acts.",
    });
    expect(within(offer).queryByRole("button", { name: "Undo all" })).toBeNull();
  });

  it("says when someone else removed the rule", async () => {
    await teachLedger(() => ({
      ...running({ state: "cancelled", changed: 0, done: 0 }),
      state: "removed",
    }));
    const offer = await screen.findByRole("dialog", {
      name: "This rule was removed, so it no longer acts.",
    });
    expect(within(offer).queryByRole("button", { name: "Undo all" })).toBeNull();
  });

  it("puts everything back with Undo all once the server agrees, then says so", async () => {
    let removed = false;
    const api = await teachLedger(
      () =>
        removed
          ? {
              ...LEDGER_RULE,
              state: "removed",
              run: { ...RUN, kind: "revert", state: "done", changed: 40 },
            }
          : LEDGER_RULE,
      {
        [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: () => {
          removed = true;
          return {
            body: {
              ...LEDGER_RULE,
              state: "removed",
              run: { ...RUN, kind: "revert", state: "running", counted: false },
            },
          };
        },
        [`POST ${W}/rules/${LEDGER_RULE.id}/restore`]: () => {
          removed = false;
          return { body: LEDGER_RULE };
        },
      },
    );
    await screen.findByRole("dialog", {
      name: "Moved 40 to Newsletters. New ones will go there too.",
    });
    fireEvent.click(screen.getByRole("button", { name: "Undo all" }));
    expect(
      await screen.findByRole("dialog", { name: "Put 40 back. The rule is removed." }),
    ).toBeVisible();
    expect(
      screen.getByText("Rule removed · Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Undo all" })).toBeNull();
    // The rule's own Undo toast cannot remove it a second time.
    expect(api.calls.filter((call) => call.route.startsWith("DELETE"))).toHaveLength(1);

    // Z brings the same rule back and applies it again: the panel follows it.
    fireEvent.keyDown(document, { key: "z" });
    expect(
      await screen.findByRole("dialog", {
        name: "Moved 40 to Newsletters. New ones will go there too.",
      }),
    ).toBeVisible();
    expect(
      api.calls.find((call) => call.route === `POST ${W}/rules/${LEDGER_RULE.id}/restore`)?.body,
    ).toEqual({ apply_to_existing: true });
  });

  it("follows the run putting mail back when the new rule's toast is undone", async () => {
    let removed = false;
    const api = await teachLedger(
      () =>
        removed
          ? {
              ...LEDGER_RULE,
              state: "removed",
              run: { ...RUN, kind: "revert", state: "running", counted: true, done: 10 },
            }
          : LEDGER_RULE,
      {
        [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: () => {
          removed = true;
          return {
            body: {
              ...LEDGER_RULE,
              state: "removed",
              run: { ...RUN, kind: "revert", state: "running", counted: true, done: 10 },
            },
          };
        },
      },
    );
    await screen.findByRole("dialog", { name: /Moved 40 to Newsletters/u });
    // Z undoes the latest action: saving the rule.
    expect(screen.getByText(/^Smart rule saved · /u)).toBeVisible();
    fireEvent.keyDown(document, { key: "z" });
    await waitFor(() => {
      expect(api.calls.some((call) => call.route.startsWith("DELETE"))).toBe(true);
    });
    expect(await screen.findByRole("dialog", { name: "Putting 40 back" })).toBeVisible();
    expect(
      await screen.findByText("Rule removed · Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    expect(screen.queryByText("This rule was removed, so it no longer acts.")).toBeNull();
    expect(api.calls.filter((call) => call.route.startsWith("DELETE"))).toHaveLength(1);
  });

  it("never asks about a rule the member may not change", async () => {
    inbox({
      [`POST ${W}/conversations/${LEDGER.id}/stream`]: (_request, body) => ({
        body: {
          ...LEDGER,
          ...(body as object),
          stream_source: "member",
          suggestion: null,
          rule_exception: { rule_id: LEDGER_RULE.id, recent_exceptions: 2 },
        },
      }),
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: {
        body: { ...LEDGER_SUGGESTIONS, suggestions: [], default: null },
      },
      [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, may_change: false } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    await screen.findByText(/Moved to Newsletters/u);
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole("dialog", { name: /Changed back/u })).toBeNull();
  });

  it("says when Undo all fails, and keeps it on offer", async () => {
    await teachLedger(() => LEDGER_RULE, {
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: problem(500, "internal"),
    });
    await screen.findByRole("dialog", { name: /Moved 40 to Newsletters/u });
    fireEvent.click(screen.getByRole("button", { name: "Undo all" }));
    expect(
      await screen.findByText("Campbooks could not undo that rule. Nothing was put back."),
    ).toBeVisible();
    expect(screen.getByRole("dialog", { name: /Moved 40 to Newsletters/u })).toBeVisible();
    expect(screen.getByRole("button", { name: "Undo all" })).toBeVisible();
  });

  it("keeps the reader's 1 from firing while the offer holds it", async () => {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: LEDGER_SUGGESTIONS },
      // The reader keeps its suggestion after the move, so both offer a 1.
      [`POST ${W}/conversations/${LEDGER.id}/stream`]: (_request, body) => ({
        body: { ...LEDGER, ...(body as object), stream_source: "member" },
      }),
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: {
        status: 201,
        body: { rule: LEDGER_RULE, acted_without_asking: false },
      },
      [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: LEDGER_RULE },
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    await screen.findByRole("dialog", { name: /Move all 40/u });
    // The reader's choice is still there, but unhinted while the offer is up.
    expect(within(reader).getByRole("button", { name: "Move to Newsletters" })).not.toHaveAttribute(
      "aria-keyshortcuts",
    );
    fireEvent.keyDown(document, { key: "1" });
    // The offer's 1 opens the smart-rule confirmation, which saves.
    const confirm = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    await within(confirm).findByText("It matches 40 emails already here.");
    fireEvent.click(within(confirm).getByRole("button", { name: /Save smart rule/u }));
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`)).toBe(
        true,
      );
    });
    const moves = api.calls.filter(
      (call) => call.route === `POST ${W}/conversations/${LEDGER.id}/stream`,
    );
    expect(moves).toHaveLength(1);
  });

  it("asks about a rule changed back twice, and removes it with Undo", async () => {
    const api = inbox({
      [`POST ${W}/conversations/${LEDGER.id}/stream`]: (_request, body) => ({
        body: {
          ...LEDGER,
          ...(body as object),
          stream_source: "member",
          suggestion: null,
          rule_exception: { rule_id: LEDGER_RULE.id, recent_exceptions: 2 },
        },
      }),
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: {
        body: { ...LEDGER_SUGGESTIONS, suggestions: [], default: null },
      },
      [`GET ${W}/rules/${LEDGER_RULE.id}`]: { body: LEDGER_RULE },
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
      [`POST ${W}/rules/${LEDGER_RULE.id}/restore`]: { body: LEDGER_RULE },
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    const question = await screen.findByRole("dialog", {
      name: "Changed back 2 times in the last 30 days. Keep the rule “Mail from hello@weekly-ledger.example → Newsletters”?",
    });
    fireEvent.click(within(question).getByRole("button", { name: "Remove the rule" }));
    expect(
      await screen.findByText("Rule removed · Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(
        api.calls.find((call) => call.route === `POST ${W}/rules/${LEDGER_RULE.id}/restore`)?.body,
      ).toEqual({ apply_to_existing: false });
    });
  });

  it("offers Stop doing this only to whoever may change the rule", async () => {
    const marked = (mayChange: boolean) => ({
      ...LEDGER_DETAIL,
      conversation: {
        ...LEDGER,
        suggestion: null,
        rule: {
          id: LEDGER_RULE.id,
          actions: LEDGER_RULE.actions,
          conditions: LEDGER_RULE.conditions,
          applied_count: 40,
          may_change: mayChange,
        },
      },
    });
    let mayChange = false;
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}`]: () => ({ body: marked(mayChange) }),
      [`POST ${W}/conversations/${LEDGER.id}/read`]: () => ({
        body: { ...marked(mayChange).conversation, unread_count: 0 },
      }),
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
    });
    const { unmount } = renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    let reader = await screen.findByRole("main", { name: "Conversation" });
    expect(
      await within(reader).findByText(
        "Campbooks moved this · like 39 others from hello@weekly-ledger.example",
      ),
    ).toBeVisible();
    expect(within(reader).queryByRole("button", { name: "Stop doing this" })).toBeNull();
    unmount();

    mayChange = true;
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    reader = await screen.findByRole("main", { name: "Conversation" });
    fireEvent.click(await within(reader).findByRole("button", { name: "Stop doing this" }));
    expect(
      await screen.findByText("Rule removed · Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    expect(api.calls.some((call) => call.route === `DELETE ${W}/rules/${LEDGER_RULE.id}`)).toBe(
      true,
    );
  });

  async function offerFor(
    suggestions: Schema<"RuleSuggestions">,
    extra: Parameters<typeof mockApi>[0] = {},
  ) {
    const api = inbox({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: suggestions },
      ...extra,
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${LEDGER.id}`);
    const reader = await screen.findByRole("main", { name: "Conversation" });
    await within(reader).findByRole("heading", { name: LEDGER.subject });
    fireEvent.keyDown(document, { key: "1" });
    return api;
  }

  const SENDER: Schema<"RuleSuggestion"> = firstOf(LEDGER_SUGGESTIONS.suggestions);
  const LIST: Schema<"RuleSuggestion"> = {
    ...SENDER,
    likeness: "list",
    key: "list|ledger",
    conditions: [{ field: "list_id", value: "Ledger <ledger.weekly-ledger.example>" }],
    preview: { ...SENDER.preview, would_change: 12 },
  };

  it("says “at least” when the preview did not see the whole inbox", async () => {
    const partial = {
      ...LEDGER_SUGGESTIONS,
      suggestions: [{ ...LIST, preview: { ...LIST.preview, would_change: 2000, complete: false } }],
    };
    await offerFor(partial);
    const offer = await screen.findByRole("dialog", {
      name: "Move all like this (at least 2,000) to Newsletters, and new ones too?",
    });
    expect(within(offer).getByRole("button", { name: "Move them all" })).toBeEnabled();
    expect(within(offer).getByText(/there may be more/u)).toBeVisible();
  });

  it("asks which mail first when there is no default, and dismisses every likeness", async () => {
    const api = await offerFor(
      { ...LEDGER_SUGGESTIONS, default: null, suggestions: [SENDER, LIST] },
      {
        [`POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`]: { status: 204 },
      },
    );
    const offer = await screen.findByRole("dialog", {
      name: "Move mail like this to Newsletters, and new ones too? Choose which.",
    });
    expect(within(offer).getByRole("button", { name: "Move them all" })).toBeDisabled();
    fireEvent.click(within(offer).getByRole("radio", { name: /Only the Ledger list/u }));
    expect(
      await screen.findByRole("dialog", {
        name: "Move all 12 like this to Newsletters, and new ones too?",
      }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Just this one/u }));
    await waitFor(() => {
      const keys = api.calls
        .filter((call) => call.route.endsWith("/rule-suggestions/dismiss"))
        .map((call) => call.body);
      expect(keys).toEqual([{ key: "sender|ledger" }, { key: "list|ledger" }]);
    });
  });

  it("says when Just this one could not be recorded", async () => {
    await offerFor(LEDGER_SUGGESTIONS, {
      [`POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`]: problem(500, "internal"),
    });
    await screen.findByRole("dialog", { name: /Move all 40/u });
    fireEvent.click(screen.getByRole("button", { name: /Just this one/u }));
    expect(
      await screen.findByText(
        "Campbooks could not record “Just this one”, so it may offer this again.",
      ),
    ).toBeVisible();
  });

  it("says when a conversation is gone, and when the list cannot load", async () => {
    inbox({
      [`GET ${W}/conversations/${Q4.id}`]: problem(404, "not_found"),
      [`GET ${W}/conversations`]: problem(503, "unavailable"),
    });
    renderAt(`/w/${DUARTE.id}/inbox/primary/${Q4.id}`);

    expect(await screen.findByRole("heading", { name: "This conversation is gone" })).toBeVisible();
    expect(await screen.findByRole("heading", { name: "Could not load Primary" })).toBeVisible();
    expect(summary(Q4.id)).toBeDefined();
  });

  it("renders in the user's language", async () => {
    inbox();
    renderAt(`/w/${DUARTE.id}/inbox/primary`, "pt-PT");

    expect(await screen.findByRole("heading", { level: 1, name: "Principal" })).toBeVisible();
    expect(await screen.findByText("Sugere Newsletters")).toBeVisible();
  });
});
