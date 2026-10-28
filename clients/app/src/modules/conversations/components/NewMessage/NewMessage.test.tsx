import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { INBOX, W } from "../../../../../test/demo/conversations";
import { NEW } from "../../../../../test/demo/drafts";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { NewMessage } from "./NewMessage";

const D = `${W}/drafts`;

describe("NewMessage", () => {
  it("starts a new message from your inbox and sends it", async () => {
    const api = mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${D}`]: { status: 201, body: NEW },
      [`GET ${D}/${NEW.id}`]: { body: NEW },
      [`PATCH ${D}/${NEW.id}`]: (_request, body) => ({
        body: { ...NEW, ...(body as object), version: 2 },
      }),
      [`POST ${D}/${NEW.id}/send`]: {
        status: 202,
        body: { ...NEW, to: [{ address: "ana@example.com" }], state: "scheduled" },
      },
    });
    const onLeave = vi.fn();
    renderWithProviders(
      <NewMessage workspaceId={DUARTE.id} onLeave={onLeave} onReopen={vi.fn()} />,
    );
    const to = await screen.findByRole("combobox", { name: "To" });
    expect(api.calls.find((call) => call.route === `POST ${D}`)?.body).toEqual({
      kind: "new",
      inbox_id: INBOX.id,
    });
    expect(screen.getByText("Sam Rivera <sam.rivera@example.com>")).toBeVisible();
    await waitFor(() => {
      expect(to).toHaveFocus();
    });
    fireEvent.change(to, { target: { value: "ana@example.com" } });
    fireEvent.keyDown(to, { key: "Enter" });
    fireEvent.change(screen.getByRole("textbox", { name: "Subject" }), {
      target: { value: "Lunch?" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/u }));
    await waitFor(() => {
      expect(onLeave).toHaveBeenCalled();
    });
    // Saved first, then sent.
    const routes = api.calls.map((call) => call.route);
    expect(routes.indexOf(`PATCH ${D}/${NEW.id}`)).toBeLessThan(
      routes.indexOf(`POST ${D}/${NEW.id}/send`),
    );
    expect(api.calls.find((call) => call.route.startsWith("PATCH"))?.body).toMatchObject({
      to: [{ address: "ana@example.com" }],
      subject: "Lunch?",
    });
    expect(screen.getByText("Sending to ana@example.com")).toBeVisible();
  });

  it("chooses the inbox to send from when there are several", async () => {
    const office = {
      ...INBOX,
      id: "0192f1a0-0000-7000-8000-0000000000b9",
      address: "office@example.com",
      display_name: null,
      role: "sender" as const,
    };
    const api = mockApi({
      [`GET ${W}/inboxes`]: {
        body: { inboxes: [INBOX, office, { ...INBOX, id: "x", role: "reader" }] },
      },
      [`POST ${D}`]: { status: 201, body: NEW },
      [`GET ${D}/${NEW.id}`]: { body: NEW },
      [`DELETE ${D}/${NEW.id}`]: { status: 204 },
    });
    renderWithProviders(
      <NewMessage workspaceId={DUARTE.id} onLeave={vi.fn()} onReopen={vi.fn()} />,
    );
    const from = await screen.findByRole("combobox", { name: "From" });
    expect(from.querySelectorAll("option")).toHaveLength(2);
    fireEvent.change(from, { target: { value: office.id } });
    await waitFor(() => {
      expect(api.calls.filter((call) => call.route === `POST ${D}`)).toHaveLength(2);
    });
    expect(api.calls.findLast((call) => call.route === `POST ${D}`)?.body).toEqual({
      kind: "new",
      inbox_id: office.id,
    });
  });

  it("asks for a mailbox when there is none to send from", async () => {
    mockApi({ [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role: "reader" }] } } });
    renderWithProviders(
      <NewMessage workspaceId={DUARTE.id} onLeave={vi.fn()} onReopen={vi.fn()} />,
    );
    expect(await screen.findByText("Connect a mailbox to send from it.")).toBeVisible();
  });

  it("reopens a draft whose send was taken back", async () => {
    const api = mockApi({
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${D}/${NEW.id}`]: { body: { ...NEW, body: "Lunch on Friday?" } },
    });
    renderWithProviders(
      <NewMessage workspaceId={DUARTE.id} draftId={NEW.id} onLeave={vi.fn()} onReopen={vi.fn()} />,
    );
    expect(await screen.findByRole("textbox", { name: "Message" })).toHaveTextContent("Lunch on Friday?");
    expect(api.calls.some((call) => call.route === `POST ${D}`)).toBe(false);
  });
});
