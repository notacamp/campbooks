import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { INBOX, LEDGER_RULE, W } from "../../../../../test/demo/conversations";
import { ANA, DUARTE, SESSION, SOFIA } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { RulesList } from "./RulesList";

const PROPOSED = {
  ...LEDGER_RULE,
  id: "0192f1a0-0000-7000-8000-0000000000f1",
  state: "proposed" as const,
  origin: "correction" as const,
  taught_by: SOFIA.user_id,
  actions: [{ action: "surface" as const, value: "never" as const }],
  conditions: [{ field: "sender_address" as const, value: "billing@hosting.example" }],
  applied_count: 0,
  last_applied_at: null,
  run: null,
};

/** Another proposal, whose author is here. */
const PROPOSED_OTHER = {
  ...PROPOSED,
  id: "0192f1a0-0000-7000-8000-0000000000f3",
  conditions: [{ field: "sender_address" as const, value: "news@daily-brief.example" }],
};

/** The first of several matches. */
function first<T>(items: readonly T[]): T {
  const [item] = items;
  if (item === undefined) {
    throw new Error("expected at least one match");
  }
  return item;
}

const people = {
  "GET /bff/session": { body: SESSION },
  [`GET ${W}/members`]: { body: { members: [ANA, SOFIA] } },
};

describe("the rules list", () => {
  it("lists rules waiting for a yes first, then each inbox's, saying who taught them", async () => {
    const api = mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [PROPOSED, LEDGER_RULE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/rules/${PROPOSED.id}/confirm`]: { body: { ...PROPOSED, state: "active" } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);

    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    expect(
      within(waiting).getByText("Mail from billing@hosting.example → never needs you"),
    ).toBeVisible();
    expect(
      await within(waiting).findByText(/Suggested from Sofia Pessoa's corrections/u),
    ).toBeVisible();
    fireEvent.click(within(waiting).getByRole("button", { name: "Keep this rule" }));
    expect(
      await screen.findByText("Rule kept · Mail from billing@hosting.example → never needs you"),
    ).toBeVisible();
    expect(api.calls.some((call) => call.route === `POST ${W}/rules/${PROPOSED.id}/confirm`)).toBe(
      true,
    );

    const mine = screen.getByRole("list", { name: INBOX.address });
    expect(
      within(mine).getByText("Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    const meta = within(mine).getByText(/40 so far/u);
    expect(meta).toHaveTextContent(/You taught this when you acted on one email/u);
    expect(meta).toHaveTextContent(/Last acted/u);
  });

  it("removes a rule with Undo, bringing the same rule back", async () => {
    const api = mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
      [`POST ${W}/rules/${LEDGER_RULE.id}/restore`]: { body: LEDGER_RULE },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const mine = await screen.findByRole("list", { name: INBOX.address });

    // Remove asks in place, and puts the mail back by default.
    fireEvent.click(within(mine).getByRole("button", { name: "Remove" }));
    expect(within(mine).getByText(/Remove this rule\?/u)).toBeVisible();
    expect(within(mine).getByText("Also put back the 40 it changed")).toBeVisible();
    fireEvent.click(within(mine).getByRole("button", { name: "Remove" }));
    expect(
      await screen.findByText("Rule removed · Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    expect(api.calls.filter((call) => call.route.startsWith("DELETE"))).toHaveLength(1);

    // Undo brings the same rule back, applied again since its mail went back.
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(
        api.calls.find((call) => call.route === `POST ${W}/rules/${LEDGER_RULE.id}/restore`)?.body,
      ).toEqual({ apply_to_existing: true });
    });
  });

  it("says when a change fails, and keeps the rule listed", async () => {
    mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [PROPOSED] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/rules/${PROPOSED.id}/confirm`]: problem(500, "internal"),
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    fireEvent.click(within(waiting).getByRole("button", { name: "Keep this rule" }));
    expect(
      await screen.findByText("Campbooks could not change that rule. Nothing changed."),
    ).toBeVisible();
    expect(within(waiting).getByRole("button", { name: "Keep this rule" })).toBeVisible();
  });

  it("keeps a rule only as it was shown: changed since, it shows again as it is", async () => {
    const shown = { ...PROPOSED, content_token: "demo-token-shown" };
    const changed = {
      ...PROPOSED,
      actions: [{ action: "trash" as const }],
      content_token: "demo-token-changed",
    };
    let listed = 0;
    const api = mockApi({
      ...people,
      [`GET ${W}/rules`]: () => {
        listed += 1;
        return { body: { rules: [listed === 1 ? shown : changed] } };
      },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/rules/${PROPOSED.id}/confirm`]: problem(409, "content_changed"),
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    fireEvent.click(within(waiting).getByRole("button", { name: "Keep this rule" }));
    expect(
      await screen.findByText("This rule changed since you opened it. Check it and keep it again."),
    ).toBeVisible();
    // The yes named what was on screen, and nothing was kept for the member.
    expect(
      api.calls.find((call) => call.route === `POST ${W}/rules/${PROPOSED.id}/confirm`)?.body,
    ).toEqual({ apply_to_existing: true, content_token: "demo-token-shown" });
    // Read again: it shows what it does now, still waiting for a yes.
    expect(
      await within(waiting).findByText("Mail from billing@hosting.example → put in the trash"),
    ).toBeVisible();
    expect(within(waiting).getByRole("button", { name: "Keep this rule" })).toBeVisible();
    expect(
      api.calls.filter((call) => call.route === `POST ${W}/rules/${PROPOSED.id}/confirm`),
    ).toHaveLength(1);
  });

  it("shows the rules while the inbox list cannot load, without naming the inbox", async () => {
    mockApi({
      [`GET ${W}/rules`]: { body: { rules: [LEDGER_RULE] } },
      [`GET ${W}/inboxes`]: problem(503, "unavailable"),
      "GET /bff/session": problem(503, "unavailable"),
      [`GET ${W}/members`]: problem(403, "forbidden"),
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const list = await screen.findByRole("list", { name: "An inbox" });
    expect(within(list).getByText(/Taught when someone acted on one email/u)).toBeVisible();
  });

  it("turns a proposal down for anyone who may teach, with Undo", async () => {
    const api = mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [{ ...PROPOSED, may_change: false }] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role: "sender" }] } },
      [`POST ${W}/rules/${PROPOSED.id}/decline`]: { body: { ...PROPOSED, state: "removed" } },
      [`POST ${W}/rules/${PROPOSED.id}/restore`]: { body: PROPOSED },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    fireEvent.click(await within(waiting).findByRole("button", { name: "No" }));
    expect(
      await screen.findByText(
        "Rule declined · Mail from billing@hosting.example → never needs you",
      ),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => {
      expect(
        api.calls.find((call) => call.route === `POST ${W}/rules/${PROPOSED.id}/restore`)?.body,
      ).toEqual({ apply_to_existing: false });
    });
  });

  it("offers Keep and No only where the member may teach", async () => {
    mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [PROPOSED] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, role: "reader" }] } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    expect(await within(waiting).findByText(/Sofia Pessoa/u)).toBeVisible();
    expect(within(waiting).queryByRole("button", { name: "Keep this rule" })).toBeNull();
    expect(within(waiting).queryByRole("button", { name: "No" })).toBeNull();
  });

  it("says when the rules cannot load, and tries again", async () => {
    let fail = true;
    mockApi({
      ...people,
      [`GET ${W}/rules`]: () => (fail ? problem(500, "internal") : { body: { rules: [] } }),
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    // Loading: the skeleton, busy.
    expect(
      screen.getAllByRole("status").some((node) => node.getAttribute("aria-busy") === "true"),
    ).toBe(true);
    expect(
      await screen.findByRole("heading", { name: "Could not load the rules" }, { timeout: 5000 }),
    ).toBeVisible();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText(/No rules yet/u)).toBeVisible();
  });

  it("explains how rules come about when there is none", async () => {
    mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    expect(await screen.findByText(/No rules yet/u)).toBeVisible();
  });

  it("says why a rule is paused", async () => {
    const suspended = {
      ...LEDGER_RULE,
      state: "paused" as const,
      paused_reason: "author_unavailable" as const,
    };
    const byMember = {
      ...PROPOSED,
      id: "0192f1a0-0000-7000-8000-0000000000f2",
      state: "paused" as const,
      paused_reason: "paused_by_member" as const,
    };
    mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [suspended, byMember] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const mine = await screen.findByRole("list", { name: INBOX.address });
    // First on its line, which is cut short on a narrow row.
    expect(within(mine).getByText(/^Paused for now; it acts again on its own · /u)).toBeVisible();
    expect(within(mine).getByText(/Paused by a member/u)).toBeVisible();
    expect(within(mine).queryByText(/can no longer change this inbox/u)).toBeNull();
  });

  it("says a rule naming a public suffix is too broad", async () => {
    const broad = {
      ...LEDGER_RULE,
      state: "paused" as const,
      paused_reason: "domain_too_broad" as const,
    };
    mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [broad] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    const mine = await screen.findByRole("list", { name: INBOX.address });
    expect(within(mine).getByText(/^Paused: its domain is too broad/u)).toBeVisible();
  });

  it("says before a yes that a proposal whose author is away stays paused", async () => {
    const away = { ...PROPOSED, paused_once_kept: "author_unavailable" as const };
    mockApi({
      ...people,
      [`GET ${W}/rules`]: { body: { rules: [away, PROPOSED_OTHER] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/rules/${PROPOSED.id}/confirm`]: {
        body: { ...away, state: "paused", paused_reason: "author_unavailable" },
      },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);

    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    const notes = within(waiting).getAllByText(
      "If kept, it stays paused until whoever taught it is back or someone takes it over",
    );
    // Only on the proposal whose author is away, and naming no cause.
    expect(notes).toHaveLength(1);
    expect(within(waiting).queryByText(/suspend/iu)).toBeNull();
    fireEvent.click(first(within(waiting).getAllByRole("button", { name: "Keep this rule" })));
    expect(
      await screen.findByText(
        "Rule kept, paused for now · Mail from billing@hosting.example → never needs you",
      ),
    ).toBeVisible();
  });
});
