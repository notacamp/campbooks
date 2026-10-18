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

  it("opens Scout's text in the composer as a saved reply draft", async () => {
    const api = routes({
      [`PATCH ${D}/${REPLY.id}`]: (_request, body) => ({
        body: { ...REPLY, ...(body as object), version: 2 },
      }),
    });
    const resume: ReplyResume = {
      draft: { ...REPLY, message_id: NEWEST, body: DRAFT },
      intent: "Say Thursday works",
    };
    const handlers = dialog({ resume });
    await screen.findByRole("textbox", { name: "Your reply" });
    fireEvent.click(screen.getByRole("button", { name: "Open in composer" }));
    await waitFor(() => {
      expect(handlers.onOpenDraft).toHaveBeenCalled();
    });
    // The undone draft is saved again, never a second one.
    expect(api.calls.some((c) => c.route === `POST ${D}`)).toBe(false);
    // Saved with exactly who the dialog shows, and no copies.
    expect(api.calls.find((c) => c.route === `PATCH ${D}/${REPLY.id}`)?.body).toEqual({
      version: 1,
      body: DRAFT,
      to: REPLY.to,
      cc: [],
      bcc: [],
    });
  });
});
