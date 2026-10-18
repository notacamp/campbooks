import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Q4, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ReplyDialog, type ReplyResume } from "./ReplyDialog";

const C = `${W}/conversations/${Q4.id}`;
const D = `${W}/drafts`;
const NEWEST = Q4_DETAIL.messages.at(-1)?.id ?? "";
const DRAFT = "Hi Maya,\n\nThursday works for me.\n\nSam Rivera";
const SHORT = "Hi Maya, Thursday works.\n\nSam";
/** The reply draft the dialog creates when it does not reuse one. */
const FRESH = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-000000000109",
  message_id: NEWEST,
  content_token: "demo-token-fresh",
};

function routes(extra: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    [`GET ${C}`]: { body: Q4_DETAIL },
    [`POST ${C}/reply/suggestions`]: {
      body: { message_id: NEWEST, intents: ["Say Thursday works", "Ask for more time"] },
    },
    [`POST ${C}/reply/draft`]: (_request, body) => ({
      body: {
        message_id: NEWEST,
        text: (body as { shorter?: boolean }).shorter === true ? SHORT : DRAFT,
      },
    }),
    [`POST ${D}`]: (_request, body) => ({
      status: 201,
      body: { ...REPLY, message_id: NEWEST, body: (body as { body: string }).body },
    }),
    [`POST ${D}/${REPLY.id}/send`]: {
      status: 202,
      body: { ...REPLY, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
    },
    [`POST ${D}/${REPLY.id}/cancel_send`]: { body: { ...REPLY, body: SHORT, version: 3 } },
    ...extra,
  });
}

function dialog(props: Partial<Parameters<typeof ReplyDialog>[0]> = {}) {
  const handlers = {
    onClose: vi.fn(),
    onWriteOwn: vi.fn(),
    onOpenDraft: vi.fn(),
    onSent: vi.fn(),
    onUndone: vi.fn(),
  };
  renderWithProviders(
    <ReplyDialog workspaceId={DUARTE.id} conversationId={Q4.id} {...handlers} {...props} />,
  );
  return handlers;
}

describe("ReplyDialog", () => {
  it("offers Scout's likely replies, writes the one picked, and sends it with undo", async () => {
    const api = routes();
    const handlers = dialog();

    expect(await screen.findByRole("dialog", { name: "Reply to Maya" })).toBeVisible();
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    expect(first).toHaveFocus();
    // Nothing is written before a pick.
    expect(api.calls.some((c) => c.route === `POST ${C}/reply/draft`)).toBe(false);

    fireEvent.keyDown(first, { key: "1" });
    const text = await screen.findByRole("textbox", { name: "Your reply" });
    expect(text).toHaveValue(DRAFT);
    expect(api.calls.find((c) => c.route === `POST ${C}/reply/draft`)?.body).toEqual({
      message_id: NEWEST,
      intent: "Say Thursday works",
    });

    fireEvent.click(screen.getByRole("button", { name: /Shorter/u }));
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Your reply" })).toHaveValue(SHORT);
    });
    expect(api.calls.findLast((c) => c.route === `POST ${C}/reply/draft`)?.body).toEqual({
      message_id: NEWEST,
      intent: "Say Thursday works",
      draft: DRAFT,
      shorter: true,
      change: null,
    });
    // No draft exists until Send.
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as {
      kind: string;
      message_id: string;
      body: string;
      quoted: string;
    };
    expect(created).toMatchObject({
      kind: "reply",
      message_id: NEWEST,
      body: SHORT,
      to: REPLY.to,
      cc: [],
      bcc: [],
    });
    expect(created.quoted).toMatch(/Maya Okafor wrote:/u);
    expect(api.calls.map((c) => c.route)).toContain(`POST ${D}/${REPLY.id}/send`);
    expect(screen.getAllByText("Sending to Maya Okafor").length).toBeGreaterThan(0);

    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(handlers.onUndone).toHaveBeenCalledWith({
        draft: { ...REPLY, body: SHORT, version: 3 },
        intent: "Say Thursday works",
      });
    });
  });

  it("quotes the words of a newest message that came without them", async () => {
    const api = routes({
      [`GET ${C}`]: {
        body: {
          ...Q4_DETAIL,
          messages: Q4_DETAIL.messages.map((message) =>
            message.id === NEWEST ? { ...message, text: null, text_unread: true } : message,
          ),
        },
      },
      [`GET ${C}/messages/${NEWEST}/body`]: {
        body: {
          text: "The harbourlight pontoon can move by two weeks.",
          html: null,
          truncated: false,
          remote_images: 0,
          images_shown: false,
          images_trusted: false,
          images_offer: false,
          images_domain: null,
        },
      },
    });
    const handlers = dialog();
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as { quoted: string };
    expect(created.quoted).toMatch(/Maya Okafor wrote:\n> The harbourlight pontoon can move/u);
  });

  it("sends nothing until the quote it could not read is read again and shown", async () => {
    let readable = false;
    const api = routes({
      [`GET ${C}`]: {
        body: {
          ...Q4_DETAIL,
          messages: Q4_DETAIL.messages.map((message) =>
            message.id === NEWEST ? { ...message, text: null, text_unread: true } : message,
          ),
        },
      },
      [`GET ${C}/messages/${NEWEST}/body`]: () =>
        readable
          ? {
              body: {
                text: "The harbourlight pontoon can move by two weeks.",
                html: null,
                truncated: false,
                remote_images: 0,
                images_shown: false,
                images_trusted: false,
                images_offer: false,
                images_domain: null,
              },
            }
          : problem(503, "unavailable"),
    });
    const handlers = dialog();
    expect(
      await screen.findByText(
        "Couldn’t read the email you’re replying to, so its quote can’t be shown. Nothing can be sent until it is.",
      ),
    ).toBeVisible();
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    // Neither Send nor its shortcut, nor Open in composer, while the quote is unread.
    expect(screen.getByRole("button", { name: /^Send/u })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Open in composer" })).toBeDisabled();
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Your reply" }), {
      key: "Enter",
      ctrlKey: true,
    });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);
    // Try again reads it, shows it, and only then may it go.
    readable = true;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(
      await screen.findByText(/The harbourlight pontoon can move by two weeks\./u),
    ).toBeVisible();
    const send = screen.getByRole("button", { name: /^Send/u });
    await waitFor(() => {
      expect(send).toBeEnabled();
    });
    fireEvent.click(send);
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as { quoted: string };
    expect(created.quoted).toMatch(/> The harbourlight pontoon can move by two weeks\./u);
  });

  it("says when the quote shown is only part of what is quoted, or all that is", async () => {
    const newest = (change: object) => ({
      [`GET ${C}`]: {
        body: {
          ...Q4_DETAIL,
          messages: Q4_DETAIL.messages.map((message) =>
            message.id === NEWEST ? { ...message, ...change } : message,
          ),
        },
      },
    });
    // A long email read in time: its reply quotes only its first part.
    routes(newest({ quote_cut: true }));
    dialog();
    expect(
      await screen.findByText("This email is long: your reply quotes its first part."),
    ).toBeVisible();
  });

  it("says when the preview shows a long quote in part", async () => {
    const words = "The timeline can move by two weeks. ".repeat(20);
    routes({
      [`GET ${C}`]: {
        body: {
          ...Q4_DETAIL,
          messages: Q4_DETAIL.messages.map((message) =>
            message.id === NEWEST
              ? { ...message, text: { content: words, truncated: false } }
              : message,
          ),
        },
      },
    });
    dialog();
    expect(await screen.findByText("Shown in part: your reply quotes all of it.")).toBeVisible();
    expect(screen.getByText(/two weeks\.[^…]*…$/u)).toBeVisible();
  });

  it("says who the reply goes to, and warns when Reply-To is elsewhere", async () => {
    const newest = Q4_DETAIL.messages.at(-1);
    if (newest === undefined) {
      throw new Error("no message");
    }
    const elsewhere = {
      ...Q4_DETAIL,
      messages: [
        ...Q4_DETAIL.messages.slice(0, -1),
        {
          ...newest,
          direction: "inbound" as const,
          reply_to: [{ name: "Billing", address: "pay@elsewhere.example" }],
        },
      ],
    };
    routes({ [`GET ${C}`]: { body: elsewhere } });
    dialog();
    expect(
      await screen.findByText(/Replies go to Billing <pay@elsewhere\.example>, not to the sender/u),
    ).toBeVisible();
  });

  it("closes with Esc and creates nothing", async () => {
    const api = routes();
    const handlers = dialog();
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(first, { key: "Escape" });
    expect(handlers.onClose).toHaveBeenCalled();
    expect(api.calls.some((c) => c.route === `POST ${D}` || c.route.endsWith("/reply/draft"))).toBe(
      false,
    );
  });

  it("offers only Write my own when no model may read mail on this server", async () => {
    routes({ [`POST ${C}/reply/suggestions`]: problem(409, "reply_unavailable") });
    const handlers = dialog();
    expect(
      await screen.findByText("Scout can't write replies on this server, so you write this one."),
    ).toBeVisible();
    expect(screen.queryByRole("textbox")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Write my own/u }));
    expect(handlers.onWriteOwn).toHaveBeenCalled();
  });

  it("keeps the member's text when a rewrite fails, and says so", async () => {
    routes({ [`POST ${C}/reply/draft`]: problem(503, "reply_failed") });
    const resume: ReplyResume = {
      draft: { ...REPLY, message_id: NEWEST, body: DRAFT },
      intent: "Say Thursday works",
    };
    dialog({ resume });
    const text = await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.change(text, { target: { value: `${DRAFT} Edited.` } });
    fireEvent.click(screen.getByRole("button", { name: /Shorter/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Scout couldn't write that just now",
    );
    expect(screen.getByRole("textbox", { name: "Your reply" })).toHaveValue(`${DRAFT} Edited.`);
  });

  it("opens Scout's text in the composer as a fresh reply draft, discarding the undone one", async () => {
    const api = routes({
      [`POST ${D}`]: (_request, body) => ({
        status: 201,
        body: { ...FRESH, body: (body as { body: string }).body },
      }),
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const resume: ReplyResume = {
      draft: { ...REPLY, message_id: NEWEST, body: DRAFT },
      intent: "Say Thursday works",
    };
    const handlers = dialog({ resume });
    await screen.findByRole("textbox", { name: "Your reply" });
    await screen.findByText("To Maya Okafor <maya@northwind-studio.example>");
    fireEvent.click(screen.getByRole("button", { name: "Open in composer" }));
    await waitFor(() => {
      expect(handlers.onOpenDraft).toHaveBeenCalled();
    });
    // The undone draft is never saved again: a new one holds what is shown,
    // with exactly who the dialog shows and no copies, and the old one goes.
    expect(api.calls.some((c) => c.route.startsWith("PATCH"))).toBe(false);
    expect(api.calls.find((c) => c.route === `POST ${D}`)?.body).toMatchObject({
      kind: "reply",
      body: DRAFT,
      to: REPLY.to,
      cc: [],
      bcc: [],
    });
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`DELETE ${D}/${REPLY.id}`);
    });
  });

  it("discards a replaced draft whose send failed: it is editable, so it goes too", async () => {
    const api = routes({
      [`POST ${D}`]: (_request, body) => ({
        status: 201,
        body: { ...FRESH, body: (body as { body: string }).body },
      }),
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const resume: ReplyResume = {
      draft: { ...REPLY, message_id: NEWEST, body: DRAFT, state: "failed" },
      intent: "Say Thursday works",
    };
    const handlers = dialog({ resume });
    await screen.findByText("To Maya Okafor <maya@northwind-studio.example>");
    fireEvent.click(screen.getByRole("button", { name: "Open in composer" }));
    await waitFor(() => {
      expect(handlers.onOpenDraft).toHaveBeenCalled();
    });
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(true);
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`DELETE ${D}/${REPLY.id}`);
    });
  });

  it("never discards a draft that is on its way: only its Undo takes a send back", async () => {
    const api = routes({
      [`POST ${D}`]: (_request, body) => ({
        status: 201,
        body: { ...FRESH, body: (body as { body: string }).body },
      }),
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    // Handed in while it is scheduled (sent again elsewhere since).
    const resume: ReplyResume = {
      draft: { ...REPLY, message_id: NEWEST, body: DRAFT, state: "scheduled" },
      intent: "Say Thursday works",
    };
    const handlers = dialog({ resume });
    await screen.findByText("To Maya Okafor <maya@northwind-studio.example>");
    fireEvent.click(screen.getByRole("button", { name: "Open in composer" }));
    await waitFor(() => {
      expect(handlers.onOpenDraft).toHaveBeenCalled();
    });
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(true);
    expect(api.calls.some((c) => c.route.startsWith("DELETE"))).toBe(false);
  });

  it("never sends what a reused draft held but the dialog did not show", async () => {
    const api = routes({
      [`POST ${D}`]: (_request, body) => ({
        status: 201,
        body: { ...FRESH, body: (body as { body: string }).body },
      }),
      [`POST ${D}/${FRESH.id}/send`]: {
        status: 202,
        body: { ...FRESH, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
      },
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    // The draft an Undo handed back now holds a Bcc and a file nobody here
    // added (what SQL could plant).
    const resume: ReplyResume = {
      draft: {
        ...REPLY,
        message_id: NEWEST,
        body: DRAFT,
        bcc: [{ name: null, address: "eve@example.com" }],
        attachments: [
          { id: "a1", filename: "invoice.pdf.exe", content_type: "application/pdf", size: 1 },
        ],
      },
      intent: "Say Thursday works",
    };
    const handlers = dialog({ resume });
    await screen.findByText("To Maya Okafor <maya@northwind-studio.example>");
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    expect(api.calls.some((c) => c.route.startsWith("PATCH"))).toBe(false);
    expect(api.calls.some((c) => c.route === `POST ${D}/${REPLY.id}/send`)).toBe(false);
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as Record<string, unknown>;
    expect(created).toMatchObject({ to: REPLY.to, cc: [], bcc: [], body: DRAFT });
    expect(created).not.toHaveProperty("attachments");
    expect(api.calls.find((c) => c.route === `POST ${D}/${FRESH.id}/send`)?.body).toEqual({
      version: FRESH.version,
      content_token: FRESH.content_token,
    });
  });
});
