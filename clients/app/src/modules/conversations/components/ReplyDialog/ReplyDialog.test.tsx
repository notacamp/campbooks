import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ScoutChatProvider } from "~/lib/scout-chat";

import { Q4, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import {
  mockApi,
  problem,
  renderWithProviders,
  replyStream,
  type MockHandler,
} from "../../../../../test/render";

import { ReplyDialog, type ReplyResume } from "./ReplyDialog";

const C = `${W}/conversations/${Q4.id}`;
const D = `${W}/drafts`;
const NEWEST = Q4_DETAIL.messages.at(-1)?.id ?? "";
const DRAFT = "Hi Maya,\n\nThursday works for me.\n\nSam Rivera";
const SHORT = "Hi Maya, Thursday works.\n\nSam";
/** What the dialog says when its draft changed since it was saved. */
const CHANGED = "This reply changed since it was saved. Check it and send again.";
/** The reply draft the dialog creates when it does not reuse one. */
const FRESH = {
  ...REPLY,
  id: "0192f1a0-0000-7000-8000-000000000109",
  message_id: NEWEST,
  content_token: "demo-token-fresh",
};

const LAST = Q4_DETAIL.messages.at(-1);
if (LAST === undefined) {
  throw new Error("the fixture has no message");
}
/** A message from Leo that arrives while the member reviews a draft. */
const NEWER = {
  ...LAST,
  id: "0192f1a0-0000-7000-8000-0000000000e9",
  from: { name: "Leo Brandt", address: "leo@northwind-studio.example" },
  reply_to: [],
  sent_at: "2026-10-13T13:00:00Z",
  text: { content: "Could we do Friday instead?", truncated: false },
  has_html: false,
  attachments: [],
};
const WITH_NEWER = { ...Q4_DETAIL, messages: [...Q4_DETAIL.messages, NEWER] };
/** The likely replies the dialog shows, sent with every draft asked for. */
const INTENTS = ["Say Thursday works", "Ask for more time"];
/** The draft route the web dialog calls: the reply streams. */
const WRITE = `POST ${C}/reply/draft/stream`;

/**
 * The streamed reply route answering what `write` answers as JSON: some
 * thinking, the words, then done (a problem stays a problem).
 */
function streamed(write: MockHandler): MockHandler {
  return (request, body) => {
    const answer = typeof write === "function" ? write(request, body) : write;
    if ((answer.status ?? 200) >= 400) {
      return answer;
    }
    const done = answer.body as { message_id: string; text: string };
    return replyStream(done.message_id, done.text);
  };
}

/** The API, its draft answers (`POST …/reply/draft`) streamed as the web dialog asks them. */
function routes(extra: Parameters<typeof mockApi>[0] = {}) {
  const all = defaults(extra);
  const draft = all[`POST ${C}/reply/draft`];
  return mockApi(
    draft === undefined || all[WRITE] !== undefined ? all : { ...all, [WRITE]: streamed(draft) },
  );
}

function defaults(extra: Parameters<typeof mockApi>[0]): Parameters<typeof mockApi>[0] {
  return {
    [`GET ${C}`]: { body: Q4_DETAIL },
    [`POST ${C}/reply/suggestions`]: { body: { message_id: NEWEST, intents: INTENTS } },
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
  };
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
  it("asks Scout about the conversation from its header, and stays open", async () => {
    routes();
    const scout = vi.fn();
    const onClose = vi.fn();
    renderWithProviders(
      <ScoutChatProvider open={scout}>
        <ReplyDialog
          workspaceId={DUARTE.id}
          conversationId={Q4.id}
          onClose={onClose}
          onWriteOwn={vi.fn()}
          onOpenDraft={vi.fn()}
        />
      </ScoutChatProvider>,
    );
    const reply = await screen.findByRole("dialog", { name: "Reply to Maya" });
    const ask = within(reply).getByRole("button", { name: "Ask Scout about this email" });

    fireEvent.click(ask);
    expect(scout).toHaveBeenCalledWith(
      { type: "conversations.conversation", id: Q4.id },
      { from: ask },
    );
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Reply to Maya" })).toBeVisible();
  });

  it("offers no Ask Scout where no chat is mounted", async () => {
    routes();
    dialog();
    const reply = await screen.findByRole("dialog", { name: "Reply to Maya" });

    expect(within(reply).queryByRole("button", { name: /Ask Scout/u })).toBeNull();
  });

  it("offers Scout's likely replies, writes the one picked, and sends it with undo", async () => {
    const api = routes();
    const handlers = dialog();

    expect(await screen.findByRole("dialog", { name: "Reply to Maya" })).toBeVisible();
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    expect(first).toHaveFocus();
    // Nothing is written before a pick.
    expect(api.calls.some((c) => c.route === WRITE)).toBe(false);

    fireEvent.keyDown(first, { key: "1" });
    const text = await screen.findByRole("textbox", { name: "Your reply" });
    expect(text).toHaveValue(DRAFT);
    expect(api.calls.find((c) => c.route === WRITE)?.body).toEqual({
      message_id: NEWEST,
      intent: "Say Thursday works",
      suggestions: INTENTS,
    });

    fireEvent.click(screen.getByRole("button", { name: /Shorter/u }));
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Your reply" })).toHaveValue(SHORT);
    });
    expect(api.calls.findLast((c) => c.route === WRITE)?.body).toEqual({
      message_id: NEWEST,
      intent: "Say Thursday works",
      suggestions: INTENTS,
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

  it("keeps answering the message its draft was written for when a newer one arrives", async () => {
    // campbooks-cloud#220: Send answers the pinned message, never mail that
    // arrived while the member reviewed the draft.
    let arrived = false;
    const api = routes({
      [`GET ${C}`]: () => ({ body: arrived ? WITH_NEWER : Q4_DETAIL }),
    });
    const handlers = {
      onClose: vi.fn(),
      onWriteOwn: vi.fn(),
      onOpenDraft: vi.fn(),
      onSent: vi.fn(),
      onUndone: vi.fn(),
    };
    const { queryClient } = renderWithProviders(
      <ReplyDialog workspaceId={DUARTE.id} conversationId={Q4.id} {...handlers} />,
    );
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    arrived = true;
    await queryClient.refetchQueries();
    expect(
      await screen.findByText(
        "A newer message from Leo Brandt arrived. Your reply still answers the one quoted below.",
      ),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as {
      message_id: string;
      quoted: string;
    };
    expect(created.message_id).toBe(NEWEST);
    expect(created.quoted).not.toContain("Friday instead");
  });

  it("answers the newer message on request, writing the draft again for it", async () => {
    let arrived = false;
    const api = routes({
      [`GET ${C}`]: () => ({ body: arrived ? WITH_NEWER : Q4_DETAIL }),
      [`POST ${C}/reply/draft`]: (_request, body) => ({
        body: {
          message_id: (body as { message_id: string }).message_id,
          text:
            (body as { message_id: string }).message_id === NEWER.id
              ? "Hi Leo, Friday works too."
              : DRAFT,
        },
      }),
    });
    const handlers = {
      onClose: vi.fn(),
      onWriteOwn: vi.fn(),
      onOpenDraft: vi.fn(),
      onSent: vi.fn(),
      onUndone: vi.fn(),
    };
    const { queryClient } = renderWithProviders(
      <ReplyDialog workspaceId={DUARTE.id} conversationId={Q4.id} {...handlers} />,
    );
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    arrived = true;
    await queryClient.refetchQueries();
    fireEvent.click(await screen.findByRole("button", { name: "Answer the newer one" }));
    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "Your reply" })).toHaveValue(
        "Hi Leo, Friday works too.",
      );
    });
    expect(api.calls.findLast((c) => c.route === WRITE)?.body).toEqual({
      message_id: NEWER.id,
      intent: "Say Thursday works",
      suggestions: INTENTS,
    });
    expect(screen.queryByText(/A newer message from/u)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    const created = api.calls.find((c) => c.route === `POST ${D}`)?.body as {
      message_id: string;
      quoted: string;
    };
    expect(created.message_id).toBe(NEWER.id);
    expect(created.quoted).toContain("Friday instead");
  });

  it("discards the draft saved for the older message once the newer one is answered and sent", async () => {
    // campbooks-cloud#257: a failed send leaves a draft for Maya's message;
    // answering Leo's instead writes a fresh one, and Maya's goes.
    let arrived = false;
    let created = 0;
    const api = routes({
      [`GET ${C}`]: () => ({ body: arrived ? WITH_NEWER : Q4_DETAIL }),
      [`POST ${C}/reply/draft`]: (_request, body) => ({
        body: { message_id: (body as { message_id: string }).message_id, text: DRAFT },
      }),
      [`POST ${D}`]: (_request, body) => {
        created += 1;
        const fields = body as { body: string; message_id: string };
        return created === 1
          ? { status: 201, body: { ...REPLY, message_id: NEWEST, body: fields.body } }
          : { status: 201, body: { ...FRESH, message_id: fields.message_id, body: fields.body } };
      },
      [`POST ${D}/${REPLY.id}/send`]: problem(503, "unavailable"),
      [`POST ${D}/${FRESH.id}/send`]: {
        status: 202,
        body: { ...FRESH, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
      },
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const handlers = {
      onClose: vi.fn(),
      onWriteOwn: vi.fn(),
      onOpenDraft: vi.fn(),
      onSent: vi.fn(),
      onUndone: vi.fn(),
    };
    const { queryClient } = renderWithProviders(
      <ReplyDialog workspaceId={DUARTE.id} conversationId={Q4.id} {...handlers} />,
    );
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`POST ${D}/${REPLY.id}/send`);
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    arrived = true;
    await queryClient.refetchQueries();
    fireEvent.click(await screen.findByRole("button", { name: "Answer the newer one" }));
    await waitFor(() => {
      expect(api.calls.findLast((c) => c.route === WRITE)?.body).toMatchObject({
        message_id: NEWER.id,
      });
    });
    const send = await screen.findByRole("button", { name: /^Send/u });
    await waitFor(() => {
      expect(send).toBeEnabled();
    });
    fireEvent.click(send);
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    expect(api.calls.filter((c) => c.route === `POST ${D}`).map((c) => c.body)).toMatchObject([
      { message_id: NEWEST },
      { message_id: NEWER.id },
    ]);
    // Maya's draft is never saved again, and goes.
    expect(api.calls.some((c) => c.route === `PATCH ${D}/${REPLY.id}`)).toBe(false);
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`DELETE ${D}/${REPLY.id}`);
    });
  });

  it("discards the draft of a failed send when Back leads to another pick", async () => {
    let created = 0;
    const api = routes({
      [`POST ${D}`]: (_request, body) => {
        created += 1;
        const fields = body as { body: string };
        return created === 1
          ? { status: 201, body: { ...REPLY, message_id: NEWEST, body: fields.body } }
          : { status: 201, body: { ...FRESH, body: fields.body } };
      },
      [`POST ${D}/${REPLY.id}/send`]: problem(503, "unavailable"),
      [`POST ${D}/${FRESH.id}/send`]: {
        status: 202,
        body: { ...FRESH, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
      },
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const handlers = dialog();
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`POST ${D}/${REPLY.id}/send`);
    });
    fireEvent.click(await screen.findByRole("button", { name: "Back to the choices" }));
    fireEvent.click(await screen.findByRole("button", { name: /Ask for more time/u }));
    await screen.findByRole("textbox", { name: "Your reply" });
    const send = screen.getByRole("button", { name: /^Send/u });
    await waitFor(() => {
      expect(send).toBeEnabled();
    });
    fireEvent.click(send);
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    expect(api.calls.filter((c) => c.route === `POST ${D}`)).toHaveLength(2);
    expect(api.calls.some((c) => c.route === `PATCH ${D}/${REPLY.id}`)).toBe(false);
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`DELETE ${D}/${REPLY.id}`);
    });
  });

  it("says nothing of a newer message that is the member's own mail", async () => {
    // First received mail: the note shows (the dialog does see newer mail);
    // then the member's own reply lands after it: the note goes, and it
    // never offers to answer the member's own mail.
    let thread = Q4_DETAIL;
    const own = {
      ...NEWER,
      id: "0192f1a0-0000-7000-8000-0000000000ea",
      direction: "outbound" as const,
      from: { name: "Sam Rivera", address: "sam@example.com" },
      to: [NEWER.from],
      sent_at: "2026-10-13T14:00:00Z",
    };
    routes({ [`GET ${C}`]: () => ({ body: thread }) });
    const { queryClient } = renderWithProviders(
      <ReplyDialog
        workspaceId={DUARTE.id}
        conversationId={Q4.id}
        onClose={vi.fn()}
        onWriteOwn={vi.fn()}
        onOpenDraft={vi.fn()}
      />,
    );
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    await screen.findByRole("textbox", { name: "Your reply" });
    thread = WITH_NEWER;
    await queryClient.refetchQueries();
    expect(await screen.findByText(/A newer message from Leo Brandt/u)).toBeVisible();

    thread = { ...WITH_NEWER, messages: [...WITH_NEWER.messages, own] };
    await queryClient.refetchQueries();
    await waitFor(() => {
      expect(screen.queryByText(/A newer message from/u)).toBeNull();
    });
    expect(screen.queryByRole("button", { name: "Answer the newer one" })).toBeNull();
    expect(screen.queryByText(/A newer message from Sam Rivera/u)).toBeNull();
  });

  it("offers no Undo when the send has no window to take it back in", async () => {
    routes({
      [`POST ${D}/${REPLY.id}/send`]: {
        status: 202,
        body: { ...REPLY, state: "scheduled", send_at: null },
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
    expect((await screen.findAllByText("Sending to Maya Okafor")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: /^Undo/u })).toBeNull();
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

  it("shows Scout thinking, then the reply typing in, then folds the thinking away", async () => {
    const frame = (event: string, data: unknown) =>
      `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
    let asked: Request | undefined;
    routes({
      [WRITE]: (request) => {
        asked = request;
        return {
          stream: [frame("thinking", { delta: "Maya asks whether Thursday works." })],
          open: true,
        };
      },
    });
    const { unmount } = renderWithProviders(
      <ReplyDialog
        workspaceId={DUARTE.id}
        conversationId={Q4.id}
        onClose={vi.fn()}
        onWriteOwn={vi.fn()}
        onOpenDraft={vi.fn()}
      />,
    );
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    expect(await screen.findByText("Maya asks whether Thursday works.")).toBeVisible();
    expect(screen.getByText("Scout is thinking…")).toBeVisible();
    // Closing stops the request in flight.
    unmount();
    expect(asked?.signal.aborted).toBe(true);
  });

  it("folds Scout's thinking away once the reply is written", async () => {
    routes();
    dialog();
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    expect(await screen.findByRole("textbox", { name: "Your reply" })).toHaveValue(DRAFT);
    const thoughts = screen.getByText("How Scout thought about it");
    expect(thoughts.closest("details")).not.toHaveAttribute("open");
    fireEvent.click(thoughts);
    expect(screen.getByText("Reading the message.")).toBeVisible();
  });

  it("says when Scout took too long, and offers the choices again", async () => {
    routes({
      [WRITE]: {
        stream: [
          `event: error\ndata: ${JSON.stringify({ status: 503, code: "reply_timed_out", title: "x", detail: "y", type: "about:blank" })}\n\n`,
        ],
      },
    });
    dialog();
    fireEvent.keyDown(await screen.findByRole("button", { name: /Say Thursday works/u }), {
      key: "1",
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Scout took too long to answer. Try again, or write it yourself.",
    );
    expect(screen.getByRole("button", { name: /Say Thursday works/u })).toBeVisible();
  });

  it("puts the member's text back when a rewrite fails after its first words", async () => {
    routes({
      [WRITE]: {
        stream: [
          `event: text\ndata: {"delta":"Hi"}\n\n`,
          `event: error\ndata: ${JSON.stringify({ status: 503, code: "reply_failed" })}\n\n`,
        ],
      },
    });
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

  it("closes with Esc and creates nothing", async () => {
    const api = routes();
    const handlers = dialog();
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(first, { key: "Escape" });
    expect(handlers.onClose).toHaveBeenCalled();
    expect(
      api.calls.some((c) => c.route === `POST ${D}` || c.route.endsWith("/reply/draft/stream")),
    ).toBe(false);
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

  it("deletes a resumed draft, closes, and brings it back with Undo", async () => {
    const api = routes({ [`DELETE ${D}/${REPLY.id}`]: { status: 204 } });
    const resume: ReplyResume = {
      draft: { ...REPLY, message_id: NEWEST, body: DRAFT },
      intent: "Say Thursday works",
    };
    const handlers = dialog({ resume });
    const text = await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.change(text, { target: { value: `${DRAFT} Edited.` } });
    fireEvent.click(screen.getByRole("button", { name: "Delete draft" }));
    await waitFor(() => {
      expect(handlers.onClose).toHaveBeenCalled();
    });
    expect(api.calls.some((c) => c.route === `DELETE ${D}/${REPLY.id}`)).toBe(true);
    expect(screen.getByText("Draft deleted")).toBeVisible();

    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(true);
    });
    expect(api.calls.find((c) => c.route === `POST ${D}`)?.body).toMatchObject({
      kind: "reply",
      message_id: NEWEST,
      body: `${DRAFT} Edited.`,
    });
  });

  it("offers Delete draft only once something is saved", async () => {
    routes({
      [`PATCH ${D}/${REPLY.id}`]: (_request, body) => ({
        body: { ...REPLY, ...(body as object), version: 2 },
      }),
    });
    dialog();
    const first = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(first, { key: "1" });
    await screen.findByRole("textbox", { name: "Your reply" });
    // Scout's text is not a draft yet.
    expect(screen.queryByRole("button", { name: "Delete draft" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open in composer" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Delete draft" })).toBeVisible();
    });
  });

  it("sends in a fresh draft when Send finds its draft saved elsewhere since", async () => {
    let created = 0;
    const api = routes({
      // The first reply draft, then a fresh one for the next Send.
      [`POST ${D}`]: (_request, body) => {
        created += 1;
        const made = created === 1 ? { ...REPLY, message_id: NEWEST } : FRESH;
        return { status: 201, body: { ...made, body: (body as { body: string }).body } };
      },
      // Saved in another tab before it could go (campbooks-cloud#242).
      [`POST ${D}/${REPLY.id}/send`]: problem(409, "draft_changed"),
      [`POST ${D}/${FRESH.id}/send`]: {
        status: 202,
        body: { ...FRESH, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
      },
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const handlers = dialog();
    const pick = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(pick, { key: "1" });
    await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    expect((await screen.findAllByText(CHANGED)).length).toBeGreaterThan(0);
    expect(handlers.onSent).not.toHaveBeenCalled();
    // The text stays, and Send writes it into a fresh draft; the old one goes.
    expect(screen.getByRole("textbox", { name: "Your reply" })).toHaveValue(DRAFT);
    fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    expect(api.calls.filter((c) => c.route === `POST ${D}`)).toHaveLength(2);
    expect(api.calls.some((c) => c.route.startsWith("PATCH"))).toBe(false);
    expect(api.calls.find((c) => c.route === `POST ${D}/${FRESH.id}/send`)?.body).toEqual({
      version: FRESH.version,
      content_token: FRESH.content_token,
    });
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`DELETE ${D}/${REPLY.id}`);
    });
  });

  it("never stays stuck when the draft it saved again was changed elsewhere since", async () => {
    let created = 0;
    let sends = 0;
    const api = routes({
      [`POST ${D}`]: (_request, body) => {
        created += 1;
        const made = created === 1 ? { ...REPLY, message_id: NEWEST } : FRESH;
        return { status: 201, body: { ...made, body: (body as { body: string }).body } };
      },
      // The first Send fails for another reason, so the dialog keeps its draft…
      [`POST ${D}/${REPLY.id}/send`]: () => {
        sends += 1;
        return problem(503, "unavailable");
      },
      // …which is saved elsewhere before the next Send saves it again.
      [`PATCH ${D}/${REPLY.id}`]: problem(409, "draft_changed"),
      [`POST ${D}/${FRESH.id}/send`]: {
        status: 202,
        body: { ...FRESH, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
      },
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const handlers = dialog();
    const pick = await screen.findByRole("button", { name: /Say Thursday works/u });
    fireEvent.keyDown(pick, { key: "1" });
    await screen.findByRole("textbox", { name: "Your reply" });
    const send = () => {
      fireEvent.click(screen.getByRole("button", { name: /^Send/u }));
    };
    send();
    await waitFor(() => {
      expect(sends).toBe(1);
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^Send/u })).toBeEnabled();
    });
    send();
    expect((await screen.findAllByText(CHANGED)).length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^Send/u })).toBeEnabled();
    });
    send();
    await waitFor(() => {
      expect(handlers.onSent).toHaveBeenCalled();
    });
    expect(api.calls.filter((c) => c.route.startsWith("PATCH"))).toHaveLength(1);
    expect(api.calls.find((c) => c.route === `POST ${D}/${FRESH.id}/send`)?.body).toEqual({
      version: FRESH.version,
      content_token: FRESH.content_token,
    });
    await waitFor(() => {
      expect(api.calls.map((c) => c.route)).toContain(`DELETE ${D}/${REPLY.id}`);
    });
  });
});

const P = `${W}/person-notes`;
const MAYA_NOTE = {
  id: "0192f1a0-0000-7000-8000-0000000002a1",
  address: "maya@northwind-studio.example",
  name: "Maya Okafor",
  note: "A client. Warm, first names, in English.",
  updated_at: "2026-10-12T09:00:00Z",
};

/** The member's notes, kept as the server would: by address, theirs alone. */
function notesApi(start: (typeof MAYA_NOTE)[] = []) {
  let notes = [...start];
  return {
    [`GET ${P}`]: () => ({ body: { notes } }),
    [`PUT ${P}`]: (_request: Request, body: unknown) => {
      const { address, note } = body as { address: string; note: string };
      const kept = {
        ...(notes.find((item) => item.address === address) ?? { ...MAYA_NOTE, address }),
        note,
      };
      notes = [kept, ...notes.filter((item) => item.address !== address)];
      return { body: kept };
    },
    [`DELETE ${P}/${MAYA_NOTE.id}`]: () => {
      notes = notes.filter((item) => item.id !== MAYA_NOTE.id);
      return { status: 204 };
    },
  };
}

describe("ReplyDialog: telling Scout about the person", () => {
  it("keeps a note about the person answered and offers to rewrite with it", async () => {
    const api = routes(notesApi());
    dialog();
    fireEvent.click(await screen.findByRole("button", { name: /Say Thursday works/u }));
    await screen.findByRole("textbox", { name: "Your reply" });

    fireEvent.click(await screen.findByRole("button", { name: /Tell Scout about Maya/u }));
    const field = screen.getByRole("textbox", { name: "About Maya, and how to write to them" });
    expect(field).toHaveFocus();
    expect(field).toHaveAttribute("placeholder", "e.g. My landlord. Formal, in Portuguese.");
    fireEvent.change(field, { target: { value: "  A client. Warm, first names.  " } });
    fireEvent.click(screen.getByRole("button", { name: /^Save/u }));

    expect(await screen.findByText("Scout knows: A client. Warm, first names.")).toBeVisible();
    // The address travels in the body, never in the URL.
    const put = api.calls.find((c) => c.route === `PUT ${P}`);
    expect(put?.body).toEqual({
      address: "maya@northwind-studio.example",
      note: "A client. Warm, first names.",
    });
    expect(api.calls.every((c) => !c.route.includes("maya@"))).toBe(true);
    // The focus is back on the line, and Saved is announced.
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Edit what Scout knows about Maya" }),
      ).toHaveFocus();
    });
    expect(screen.getAllByRole("status").some((s) => s.textContent.includes("Saved."))).toBe(true);
    expect(await screen.findByRole("button", { name: /^Undo/u })).toBeInTheDocument();

    // Nothing is written again on its own: only on Rewrite with this.
    const writes = () => api.calls.filter((c) => c.route === WRITE).length;
    expect(writes()).toBe(1);
    fireEvent.click(screen.getByRole("button", { name: /Rewrite with this/u }));
    await waitFor(() => {
      expect(writes()).toBe(2);
    });
    expect(api.calls.findLast((c) => c.route === WRITE)?.body).toEqual({
      message_id: NEWEST,
      intent: "Say Thursday works",
      suggestions: INTENTS,
    });
  });

  it("leaves the field with Esc and keeps the note with ⌘↵, never sending", async () => {
    const api = routes(notesApi());
    const handlers = dialog();
    fireEvent.click(await screen.findByRole("button", { name: /Say Thursday works/u }));
    await screen.findByRole("textbox", { name: "Your reply" });

    fireEvent.click(await screen.findByRole("button", { name: /Tell Scout about Maya/u }));
    let field = screen.getByRole("textbox", { name: "About Maya, and how to write to them" });
    fireEvent.change(field, { target: { value: "Not this" } });
    fireEvent.keyDown(field, { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: /About Maya/u })).toBeNull();
    // The dialog stays, and the focus is back on the line.
    expect(handlers.onClose).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Tell Scout about Maya/u })).toHaveFocus();
    });

    fireEvent.click(screen.getByRole("button", { name: /Tell Scout about Maya/u }));
    field = screen.getByRole("textbox", { name: "About Maya, and how to write to them" });
    expect(field).toHaveValue("");
    fireEvent.change(field, { target: { value: "A client." } });
    fireEvent.keyDown(field, { key: "Enter", metaKey: true });
    expect(await screen.findByText("Scout knows: A client.")).toBeVisible();
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);
    expect(handlers.onSent).not.toHaveBeenCalled();
  });

  it("shows what Scout knows, and forgets it with Undo", async () => {
    const api = routes(notesApi([MAYA_NOTE]));
    dialog();
    expect(
      await screen.findByText("Scout knows: A client. Warm, first names, in English."),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Edit what Scout knows about Maya" }));
    expect(
      screen.getByRole("textbox", { name: "About Maya, and how to write to them" }),
    ).toHaveValue(MAYA_NOTE.note);
    fireEvent.click(screen.getByRole("button", { name: "Forget this" }));
    expect(await screen.findByRole("button", { name: /Tell Scout about Maya/u })).toBeVisible();
    expect(api.calls.map((c) => c.route)).toContain(`DELETE ${P}/${MAYA_NOTE.id}`);

    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(api.calls.findLast((c) => c.route === `PUT ${P}`)?.body).toEqual({
        address: MAYA_NOTE.address,
        note: MAYA_NOTE.note,
      });
    });
  });

  it("says why a note can't be kept", async () => {
    routes({
      ...notesApi(),
      [`PUT ${P}`]: problem(409, "person_notes_full"),
    });
    dialog();
    fireEvent.click(await screen.findByRole("button", { name: /Tell Scout about Maya/u }));
    fireEvent.change(screen.getByRole("textbox", { name: /About Maya/u }), {
      target: { value: "A client." },
    });
    fireEvent.click(screen.getByRole("button", { name: /^Save/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "You keep 500 notes about people already.",
    );
    expect(screen.getByRole("textbox", { name: /About Maya/u })).toHaveValue("A client.");
  });

  it("offers nothing to tell when Scout can't write replies here", async () => {
    routes({
      ...notesApi(),
      [`POST ${C}/reply/suggestions`]: problem(409, "reply_unavailable"),
    });
    dialog();
    await screen.findByText("Scout can't write replies on this server, so you write this one.");
    expect(screen.queryByRole("button", { name: /Tell Scout about/u })).toBeNull();
  });
});
