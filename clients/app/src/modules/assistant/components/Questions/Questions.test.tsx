import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { mockApi, renderWithProviders } from "../../../../../test/render";

import { Questions } from "./Questions";

const WS = "ws-1";
const W = `/bff/api/v1/w/${WS}`;
const PROFILE = { proactivity: "ask_first", asked_at: null, revision: 0 };
const INBOXES = [
  {
    id: "inbox-own",
    channel: "email",
    address: "ines@ateliersousa.example",
    role: "manager",
    unread_count: 0,
    shared: false,
  },
  {
    id: "inbox-team",
    channel: "email",
    address: "info@ateliersousa.example",
    role: "sender",
    unread_count: 0,
    shared: true,
  },
  {
    id: "inbox-books",
    channel: "email",
    address: "books@ateliersousa.example",
    role: "reader",
    unread_count: 0,
    shared: true,
  },
];
const SENDERS = [
  { address: "rita@silva-contas.example", name: "Rita Gomes", received: 41, replied: 23 },
  { address: "duarte@pessoa-obras.example", name: "Duarte Pessoa", received: 28, replied: 17 },
];
const PREVIEW = {
  matching: 41,
  would_change: 41,
  samples: [
    {
      conversation_id: "c-1",
      subject: "Q3 VAT figures",
      sender: "rita@silva-contas.example",
      sent_at: "2026-10-12T09:00:00Z",
    },
  ],
  skipped_suspicious: 0,
  skipped_member: 0,
  overrides: [],
};

function mockEverything() {
  let next = 0;
  return mockApi({
    [`GET ${W}/inboxes`]: { body: { inboxes: INBOXES } },
    [`GET ${W}/senders`]: { body: { senders: SENDERS } },
    [`GET ${W}/assistant/profile`]: { body: PROFILE },
    [`PUT ${W}/assistant/profile`]: (_request, body) => ({
      body: { ...(body as object), asked_at: "2026-10-14T10:00:00Z", revision: 1 },
    }),
    [`POST ${W}/inboxes/inbox-own/rules/preview`]: { body: PREVIEW },
    [`POST ${W}/inboxes/inbox-team/rules/preview`]: {
      body: { ...PREVIEW, matching: 3, samples: [] },
    },
    [`POST ${W}/inboxes/inbox-own/rules`]: () => {
      next += 1;
      return {
        status: 201,
        body: { rule: { id: `rule-${String(next)}` }, acted_without_asking: false },
      };
    },
    [`POST ${W}/inboxes/inbox-team/rules`]: () => {
      next += 1;
      return {
        status: 201,
        body: { rule: { id: `rule-${String(next)}` }, acted_without_asking: false },
      };
    },
  });
}

describe("Questions", () => {
  it("turns answers into rules with their matches, own inboxes only unless a shared one is switched on", async () => {
    const api = mockEverything();
    const onHome = vi.fn();
    renderWithProviders(<Questions workspaceId={WS} onHome={onHome} onMemory={vi.fn()} />);

    expect(
      await screen.findByRole("heading", { name: "Whose mail should always reach you?" }),
    ).toBeVisible();
    expect(screen.getByText("1 of 3")).toBeVisible();
    fireEvent.click(await screen.findByRole("checkbox", { name: "Rita Gomes" }));
    expect(
      await screen.findByText(
        "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox and never gets filed away.",
      ),
    ).toBeVisible();
    expect(await screen.findByText("Matches 41 emails")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Next/u }));

    expect(await screen.findByRole("heading", { name: "What never needs you?" })).toBeVisible();
    fireEvent.click(screen.getByRole("checkbox", { name: "Newsletters and mailing lists" }));
    expect(await screen.findByText("Newsletters and mailing lists never need you.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Next/u }));

    expect(
      await screen.findByRole("heading", { name: "When Scout is sure, should it act?" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("radio", { name: "Do it and tell me" }));
    fireEvent.click(screen.getByRole("button", { name: /Review/u }));

    expect(await screen.findByRole("heading", { name: "Here’s what Scout will do" })).toBeVisible();
    expect(
      screen.getByText("books@ateliersousa.example: you can only read it, so no rules there."),
    ).toBeVisible();
    const team = screen.getByRole("checkbox", { name: "info@ateliersousa.example" });
    expect(team).not.toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: /Save/u }));

    expect(
      await screen.findByRole("heading", { name: "Scout knows what you care about" }),
    ).toBeVisible();
    const created = api.calls.filter(
      (call) => call.route.startsWith("POST") && call.route.endsWith("/rules"),
    );
    expect(created.map((call) => call.route)).toEqual([
      `POST ${W}/inboxes/inbox-own/rules`,
      `POST ${W}/inboxes/inbox-own/rules`,
    ]);
    expect(created[0]?.body).toMatchObject({
      conditions: [{ field: "sender_address", value: "rita@silva-contas.example" }],
      actions: [{ action: "surface", value: "always" }],
      origin: "onboarding",
      state: "active",
      apply_to_existing: true,
    });
    const profile = api.calls.find((call) => call.route === `PUT ${W}/assistant/profile`);
    expect(profile?.body).toEqual({ proactivity: "act_then_tell", asked: true });
    expect(screen.getByText("Saved 2 rules and your answers")).toBeVisible();
  });

  it("skips every question with S and saves only that they were asked", async () => {
    const api = mockEverything();
    renderWithProviders(<Questions workspaceId={WS} onHome={vi.fn()} onMemory={vi.fn()} />);

    await screen.findByRole("heading", { name: "Whose mail should always reach you?" });
    fireEvent.keyDown(document.body, { key: "s" });
    await screen.findByRole("heading", { name: "What never needs you?" });
    fireEvent.keyDown(document.body, { key: "s" });
    await screen.findByRole("heading", { name: "When Scout is sure, should it act?" });
    fireEvent.keyDown(document.body, { key: "s" });

    expect(
      await screen.findByText("No rules. Scout keeps sorting your mail as it does today."),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Save/u }));
    await screen.findByRole("heading", { name: "Scout knows what you care about" });
    expect(
      api.calls.some((call) => call.route.startsWith("POST") && call.route.endsWith("/rules")),
    ).toBe(false);
  });

  it("Not now (Esc) records only that the questions were put off, then goes Home", async () => {
    const api = mockEverything();
    const onHome = vi.fn();
    renderWithProviders(<Questions workspaceId={WS} onHome={onHome} onMemory={vi.fn()} />);

    await screen.findByRole("heading", { name: "Whose mail should always reach you?" });
    fireEvent.keyDown(document.body, { key: "Escape" });

    await waitFor(() => {
      expect(onHome).toHaveBeenCalled();
    });
    const put = api.calls.find((call) => call.route === `PUT ${W}/assistant/profile`);
    expect(put?.body).toEqual({ proactivity: "ask_first", asked: true });
  });

  it("refuses a free mail domain with why", async () => {
    mockEverything();
    renderWithProviders(<Questions workspaceId={WS} onHome={vi.fn()} onMemory={vi.fn()} />);

    const field = await screen.findByRole("textbox", { name: "Someone else" });
    fireEvent.change(field, { target: { value: "gmail.com" } });
    fireEvent.keyDown(field, { key: "Enter" });

    expect(await screen.findByText(/gmail\.com is a free mail provider/u)).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "Whose mail should always reach you?" }),
    ).toBeVisible();
  });

  it("moves on with Enter after ticking a box", async () => {
    mockEverything();
    renderWithProviders(<Questions workspaceId={WS} onHome={vi.fn()} onMemory={vi.fn()} />);

    const rita = await screen.findByRole("checkbox", { name: "Rita Gomes" });
    fireEvent.click(rita);
    fireEvent.keyDown(rita, { key: "Enter" });

    expect(await screen.findByRole("heading", { name: "What never needs you?" })).toBeVisible();
  });
});
