import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { INBOX, LEDGER_RULE, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { RulesList } from "./RulesList";

const PROPOSED = {
  ...LEDGER_RULE,
  id: "0192f1a0-0000-7000-8000-0000000000f1",
  state: "proposed" as const,
  origin: "correction" as const,
  actions: [{ action: "surface" as const, value: "never" as const }],
  conditions: [{ field: "sender_address" as const, value: "billing@hosting.example" }],
  applied_count: 0,
  run: null,
};

describe("the rules list", () => {
  it("lists rules waiting for a yes first, then each inbox's, as sentences", async () => {
    const api = mockApi({
      [`GET ${W}/rules`]: { body: { rules: [PROPOSED, LEDGER_RULE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/rules/${PROPOSED.id}/confirm`]: { body: { ...PROPOSED, state: "active" } },
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);

    const waiting = await screen.findByRole("list", { name: "Waiting for you" });
    expect(
      within(waiting).getByText("Mail from billing@hosting.example → never needs you"),
    ).toBeVisible();
    expect(within(waiting).getByText(/Suggested from your corrections/u)).toBeVisible();
    fireEvent.click(within(waiting).getByRole("button", { name: "Keep this rule" }));
    await waitFor(() => {
      expect(
        api.calls.some((call) => call.route === `POST ${W}/rules/${PROPOSED.id}/confirm`),
      ).toBe(true);
    });

    const mine = screen.getByRole("list", { name: INBOX.address });
    expect(
      within(mine).getByText("Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    expect(within(mine).getByText(/40 so far/u)).toBeVisible();

    // Remove asks in place, and puts the mail back by default.
    fireEvent.click(within(mine).getByRole("button", { name: "Remove" }));
    expect(within(mine).getByText(/Remove this rule\?/u)).toBeVisible();
    fireEvent.click(within(mine).getByRole("button", { name: "Remove" }));
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === `DELETE ${W}/rules/${LEDGER_RULE.id}`)).toBe(
        true,
      );
    });
  });

  it("explains how rules come about when there is none", async () => {
    mockApi({
      [`GET ${W}/rules`]: { body: { rules: [] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<RulesList workspaceId={DUARTE.id} />);
    expect(await screen.findByText(/No rules yet/u)).toBeVisible();
  });
});
