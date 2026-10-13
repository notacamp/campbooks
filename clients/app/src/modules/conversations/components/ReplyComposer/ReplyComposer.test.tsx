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
      await screen.findByText("Gmail refused this message. Check the addresses and try again."),
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

  it("says so when a draft cannot be started", async () => {
    mockApi({
      [`GET ${D}`]: { body: { drafts: [] } },
      [`POST ${D}`]: problem(403, "inbox_read_only"),
    });
    renderWithProviders(slot({ mode: "reply_all", messageId: FIRST, nonce: 1 }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Something went wrong");
  });
});
