import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { INBOX, Q4, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { REPLY } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";
import type { ReplyRequest } from "../Reader/ReplySlot";

import { ReplyComposer } from "./ReplyComposer";

const D = `${W}/drafts`;
const FIRST = Q4_DETAIL.messages[0]?.id ?? "";

function slot(request: ReplyRequest | null, onClose = vi.fn()) {
  return (
    <ReplyComposer
      workspaceId={DUARTE.id}
      inboxId={INBOX.id}
      conversation={Q4}
      messages={Q4_DETAIL.messages}
      request={request}
      onClose={onClose}
    />
  );
}

describe("ReplyComposer", () => {
  it("starts a reply with the quoted history, saves as you type, sends with undo", async () => {
    const stored: (typeof REPLY)[] = [];
    const api = mockApi({
      [`GET ${D}`]: () => ({ body: { drafts: stored } }),
      [`POST ${D}`]: () => {
        stored.push(REPLY);
        return { status: 201, body: REPLY };
      },
      [`PATCH ${D}/${REPLY.id}`]: (_request, body) => ({
        body: { ...REPLY, ...(body as object), version: 2 },
      }),
      [`POST ${D}/${REPLY.id}/send`]: {
        status: 202,
        body: { ...REPLY, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
      },
      [`POST ${D}/${REPLY.id}/cancel_send`]: { body: { ...REPLY, version: 3 } },
    });
    const onClose = vi.fn();
    renderWithProviders(slot({ mode: "reply", messageId: FIRST, nonce: 1 }, onClose));

    const box = await screen.findByRole("textbox", { name: "Message" });
    expect(screen.getByRole("region", { name: "Reply to Maya Okafor" })).toBeVisible();
    const created = api.calls.find((call) => call.route === `POST ${D}`);
    expect(created?.body).toMatchObject({ kind: "reply", message_id: FIRST });
    expect((created?.body as { quoted: string }).quoted).toMatch(
      /Maya Okafor wrote:\n> Hi Sam, here is the first draft/u,
    );

    fireEvent.change(box, { target: { value: "Thursday works." } });
    await waitFor(
      () => {
        expect(screen.getByText("Saved")).toBeVisible();
      },
      { timeout: 3000 },
    );
    expect(api.calls.find((call) => call.route.startsWith("PATCH"))?.body).toMatchObject({
      version: 1,
      body: "Thursday works.",
    });

    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    const undo = await screen.findByRole("button", { name: /Undo/u });
    expect(screen.getByText("Sending to Maya Okafor")).toBeVisible();
    expect(onClose).toHaveBeenCalled();
    fireEvent.click(undo);
    await waitFor(() => {
      expect(api.calls.some((call) => call.route.endsWith("/cancel_send"))).toBe(true);
    });
  });

  it("reopens the draft already started for that message, and shows why a send failed", async () => {
    mockApi({
      [`GET ${D}`]: {
        body: { drafts: [{ ...REPLY, state: "failed", failure_reason: "rejected" }] },
      },
    });
    renderWithProviders(slot(null));
    expect(
      await screen.findByText(
        "Your mail provider refused this message. Check the addresses and try again.",
      ),
    ).toBeVisible();
    // Esc closes it, and the saved draft stays a click away.
    const box = screen.getByRole("textbox", { name: "Message" });
    // Once the composer's shortcuts are live (it focused itself on opening).
    await waitFor(() => {
      fireEvent.keyDown(box, { key: "Escape" });
      expect(screen.getByRole("button", { name: "Draft saved" })).toBeVisible();
    });
    fireEvent.click(screen.getByRole("button", { name: "Draft saved" }));
    expect(await screen.findByRole("textbox", { name: "Message" })).toBeVisible();
  });

  it("explains a refused send and discards on request", async () => {
    const api = mockApi({
      [`GET ${D}`]: { body: { drafts: [REPLY] } },
      [`POST ${D}/${REPLY.id}/send`]: problem(400, "no_recipients"),
      [`DELETE ${D}/${REPLY.id}`]: { status: 204 },
    });
    const onClose = vi.fn();
    renderWithProviders(slot({ mode: "reply", messageId: FIRST, nonce: 1 }, onClose));
    fireEvent.click(await screen.findByRole("button", { name: /Send/u }));
    expect(await screen.findByText("Add at least one recipient.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await waitFor(() => {
      expect(onClose).toHaveBeenCalled();
    });
    expect(api.calls.some((call) => call.route.startsWith("DELETE"))).toBe(true);
    expect(api.calls.some((call) => call.route === `POST ${D}`)).toBe(false);
  });

  it("forwards with the subject and attachments", async () => {
    const forward = {
      ...REPLY,
      kind: "forward" as const,
      to: [],
      subject: `Fwd: ${Q4.subject}`,
      attachments: [
        { id: "a1", filename: "q4-plan-draft.pdf", content_type: "application/pdf", size: 48_000 },
      ],
    };
    const stored: (typeof forward)[] = [];
    const api = mockApi({
      [`GET ${D}`]: () => ({ body: { drafts: stored } }),
      [`POST ${D}`]: () => {
        stored.push(forward);
        return { status: 201, body: forward };
      },
    });
    renderWithProviders(slot({ mode: "forward", messageId: FIRST, nonce: 1 }));
    expect(await screen.findByRole("region", { name: "Forward" })).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Subject" })).toHaveValue(`Fwd: ${Q4.subject}`);
    expect(screen.getByText("q4-plan-draft.pdf")).toBeVisible();
    expect(
      (api.calls.find((call) => call.route === `POST ${D}`)?.body as { quoted: string }).quoted,
    ).toMatch(/^---------- Forwarded message ----------/u);
  });

  it("sends nothing when the last changes could not be saved", async () => {
    const api = mockApi({
      [`GET ${D}`]: { body: { drafts: [REPLY] } },
      [`PATCH ${D}/${REPLY.id}`]: { status: 503, body: { code: "unavailable" } },
      [`POST ${D}/${REPLY.id}/send`]: { status: 202, body: { ...REPLY, state: "scheduled" } },
    });
    renderWithProviders(slot({ mode: "reply", messageId: FIRST, nonce: 1 }));
    const box = await screen.findByRole("textbox", { name: "Message" });
    // Leo was removed just before sending, but the save failed: the server
    // still has him, so nothing may be sent.
    fireEvent.change(box, { target: { value: "Without Leo." } });
    fireEvent.click(screen.getByRole("button", { name: /Send/u }));
    expect(
      await screen.findByText(
        "Your last changes could not be saved, so nothing was sent. Try again.",
      ),
    ).toBeVisible();
    expect(api.calls.some((call) => call.route.endsWith("/send"))).toBe(false);
  });

  it("sends the version it saved, and says when Undo came too late", async () => {
    const api = mockApi({
      [`GET ${D}`]: { body: { drafts: [{ ...REPLY, version: 4 }] } },
      [`POST ${D}/${REPLY.id}/send`]: {
        status: 202,
        body: {
          ...REPLY,
          version: 4,
          state: "scheduled",
          send_at: new Date(Date.now() + 10_000).toISOString(),
        },
      },
      [`POST ${D}/${REPLY.id}/cancel_send`]: problem(409, "too_late"),
    });
    renderWithProviders(slot({ mode: "reply", messageId: FIRST, nonce: 1 }));
    fireEvent.click(await screen.findByRole("button", { name: /Send/u }));
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    expect(await screen.findByText("Too late to undo: it was sent.")).toBeVisible();
    expect(api.calls.find((call) => call.route.endsWith("/send"))?.body).toEqual({
      version: 4,
      content_token: REPLY.content_token,
    });
  });

  it("shows the draft as it is now when it changed since it was shown, and sends only on a new Send", async () => {
    const changed = {
      ...REPLY,
      bcc: [{ name: null, address: "eve@example.com" }],
      content_token: "demo-token-changed",
    };
    let sends = 0;
    const api = mockApi({
      [`GET ${D}`]: { body: { drafts: [REPLY] } },
      [`GET ${D}/${REPLY.id}`]: { body: changed },
      [`POST ${D}/${REPLY.id}/send`]: () => {
        sends += 1;
        return sends === 1
          ? problem(409, "content_changed")
          : {
              status: 202,
              body: { ...changed, state: "scheduled", send_at: "2026-10-13T15:00:10Z" },
            };
      },
    });
    renderWithProviders(slot({ mode: "reply", messageId: FIRST, nonce: 1 }));
    fireEvent.click(await screen.findByRole("button", { name: /Send/u }));
    expect(
      await screen.findByText("This changed since you opened it. Check it and send again."),
    ).toBeVisible();
    // The Bcc it gained is on screen, and nothing was sent for the member.
    expect(await screen.findByText("eve@example.com")).toBeVisible();
    expect(sends).toBe(1);
    fireEvent.click(screen.getByRole("button", { name: /Send/u }));
    await waitFor(() => {
      expect(sends).toBe(2);
    });
    expect(
      api.calls.filter((call) => call.route.endsWith("/send")).map((call) => call.body),
    ).toEqual([
      { version: 1, content_token: REPLY.content_token },
      { version: 1, content_token: "demo-token-changed" },
    ]);
  });

  it("says so when a draft cannot be started", async () => {
    mockApi({
      [`GET ${D}`]: { body: { drafts: [] } },
      [`POST ${D}`]: problem(403, "inbox_read_only"),
    });
    renderWithProviders(slot({ mode: "reply_all", messageId: FIRST, nonce: 1 }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Something went wrong");
  });
});
