import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import {
  INBOX,
  LEDGER,
  LEDGER_RULE,
  LEDGER_SUGGESTIONS,
  W,
} from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import type { Schema } from "~/lib/api";

import { useAheadOffer } from "./AheadOffer";
import { ScoutNoticed } from "./ScoutNoticed";
import { SmartRuleConfirm, type SmartRuleDraft } from "./SmartRuleConfirm";

const PREVIEW = LEDGER_SUGGESTIONS.suggestions[0]?.preview;

const DRAFT: SmartRuleDraft = {
  inboxId: INBOX.id,
  conditions: [{ field: "sender_address", value: "hello@weekly-ledger.example" }],
  actions: [{ action: "move_to_stream", stream: "newsletters" }],
  title: "A rule for mail like this",
};

const NOTICE: Schema<"RuleNotice"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e1",
  inbox_id: INBOX.id,
  kind: "keep_off_home",
  sender: "builds@ci.example.com",
  sender_name: "CI",
  conditions: [{ field: "sender_address", value: "builds@ci.example.com" }],
  actions: [
    { action: "move_to_stream", stream: "updates" },
    { action: "surface", value: "never" },
  ],
  evidence: {
    received: 34,
    opened: 2,
    archived_unread: 0,
    last_week: 7,
    window_days: 60,
    cadence: "daily",
  },
  shared: false,
};

const created = {
  status: 201,
  body: { rule: LEDGER_RULE, acted_without_asking: false },
};

describe("the smart-rule confirmation", () => {
  it("shows the rule as a smart rule and saves nothing before Save smart rule", async () => {
    const api = mockApi({
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: created,
      [`DELETE ${W}/rules/${LEDGER_RULE.id}`]: { body: { ...LEDGER_RULE, state: "removed" } },
    });
    const onClose = vi.fn();
    renderWithProviders(
      <SmartRuleConfirm workspaceId={DUARTE.id} draft={DRAFT} onClose={onClose} />,
    );
    const dialog = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    expect(within(dialog).getByText("Smart rule")).toBeVisible();
    expect(
      within(dialog).getByText("Mail from hello@weekly-ledger.example → Newsletters"),
    ).toBeVisible();
    expect(within(dialog).getByText(/runs on its own from now on, with no AI/u)).toBeVisible();
    expect(await within(dialog).findByText("It matches 40 emails already here.")).toBeVisible();
    expect(within(dialog).getByText("What changes for freelancers in 2027")).toBeVisible();
    const existing = within(dialog).getByRole("checkbox", {
      name: "Also apply it to the 40 already here",
    });
    expect(existing).toBeChecked();
    expect(api.calls.some((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`)).toBe(
      false,
    );

    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    expect(await screen.findByText("Smart rule saved")).toBeVisible();
    const made = api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`);
    expect(made?.body).toMatchObject({
      conditions: DRAFT.conditions,
      actions: DRAFT.actions,
      apply_to_existing: true,
    });

    // The toast's Undo removes it and puts back what it changed.
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(api.calls.some((call) => call.route === `DELETE ${W}/rules/${LEDGER_RULE.id}`)).toBe(
        true,
      );
    });
  });

  it("leaves the mail already here alone when unchecked, and Cancel saves nothing", async () => {
    const api = mockApi({
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: created,
    });
    const onClose = vi.fn();
    const { unmount } = renderWithProviders(
      <SmartRuleConfirm workspaceId={DUARTE.id} draft={DRAFT} onClose={onClose} />,
    );
    const dialog = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    fireEvent.click(
      await within(dialog).findByRole("checkbox", { name: "Also apply it to the 40 already here" }),
    );
    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    await waitFor(() => {
      expect(
        api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`)?.body,
      ).toMatchObject({ apply_to_existing: false });
    });
    unmount();

    const again = mockApi({
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
    });
    const close = vi.fn();
    renderWithProviders(<SmartRuleConfirm workspaceId={DUARTE.id} draft={DRAFT} onClose={close} />);
    const second = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    fireEvent.click(within(second).getByRole("button", { name: /^Cancel/u }));
    expect(close).toHaveBeenCalled();
    expect(again.calls.some((call) => call.route.endsWith("/rules"))).toBe(false);
  });

  it("says a failed save in the dialog and keeps it open", async () => {
    mockApi({
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: { status: 500, body: { code: "internal" } },
    });
    renderWithProviders(
      <SmartRuleConfirm workspaceId={DUARTE.id} draft={DRAFT} onClose={vi.fn()} />,
    );
    const dialog = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    await within(dialog).findByText("It matches 40 emails already here.");
    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    expect(
      await within(dialog).findByText("Campbooks could not save this smart rule. Nothing changed."),
    ).toBeVisible();
    expect(within(dialog).getByRole("button", { name: /Try again/u })).toBeVisible();
  });
});

describe("Scout noticed", () => {
  it("offers one suggestion, with Not now and its Undo, and sets up a smart rule", async () => {
    const api = mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/rule-notices/${NOTICE.id}/answer`]: { status: 204 },
      [`DELETE ${W}/rule-notices/${NOTICE.id}/answer`]: { status: 204 },
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    const note = await screen.findByRole("region", { name: "Scout noticed" });
    expect(
      within(note).getByText(
        /You got 34 emails from CI in the last 60 days and opened 2\. They come most days\. Keep them off Home and file them under Updates\?/u,
      ),
    ).toBeVisible();
    expect(within(note).getByText("1 of 1")).toBeVisible();

    fireEvent.click(within(note).getByRole("button", { name: "Set up a smart rule…" }));
    const dialog = await screen.findByRole("dialog", { name: "A rule for mail from CI" });
    fireEvent.click(within(dialog).getByRole("button", { name: /^Cancel/u }));

    fireEvent.click(within(note).getByRole("button", { name: "Not now" }));
    await waitFor(() => {
      expect(screen.queryByRole("region", { name: "Scout noticed" })).toBeNull();
    });
    expect(
      api.calls.find((call) => call.route === `POST ${W}/rule-notices/${NOTICE.id}/answer`)?.body,
    ).toEqual({ answer: "not_now" });
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(
        api.calls.some((call) => call.route === `DELETE ${W}/rule-notices/${NOTICE.id}/answer`),
      ).toBe(true);
    });
  });

  it("shows nothing while there is nothing to say", async () => {
    mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole("region", { name: "Scout noticed" })).toBeNull();
  });
});

const REPORT_OFFER: Schema<"RuleSuggestions"> = {
  shared: false,
  default: 0,
  none: null,
  suggestions: [
    {
      likeness: "impersonation",
      key: "impersonation|paypal",
      conditions: [
        { field: "sender_domain", value: "paypal.com" },
        { field: "sender_checks_fail" },
      ],
      actions: [{ action: "spam" }],
      preview: {
        matching: 0,
        would_change: 0,
        skipped_suspicious: 0,
        skipped_member: 0,
        overrides: [],
        complete: true,
        samples: [],
      },
    },
  ],
};

/** A screen that offers after Report and delete on the Ledger. */
function Reported() {
  const ahead = useAheadOffer(DUARTE.id);
  const [asked, setAsked] = useState(false);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setAsked(true);
          ahead.offer(LEDGER, { kind: "report" });
        }}
      >
        report
      </button>
      {asked ? ahead.element : null}
    </>
  );
}

describe("the offer after Report and delete", () => {
  it("asks about mail pretending to be the sender, and Yes opens the confirmation", async () => {
    const api = mockApi({
      [`GET ${W}/conversations/${LEDGER.id}/rule-suggestions`]: { body: REPORT_OFFER },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: {
        body: REPORT_OFFER.suggestions[0]?.preview,
      },
      [`POST ${W}/conversations/${LEDGER.id}/rule-suggestions/dismiss`]: { status: 204 },
    });
    renderWithProviders(<Reported />);
    fireEvent.click(screen.getByRole("button", { name: "report" }));
    const offer = await screen.findByRole("dialog", { name: "Never see emails like this again?" });
    expect(
      within(offer).getByText(/says it’s from paypal\.com but fails its sender checks/u),
    ).toBeVisible();
    expect(api.calls.find((call) => call.route.endsWith("/rule-suggestions"))?.route).toBeDefined();
    fireEvent.click(within(offer).getByRole("button", { name: /Yes, from now on/u }));
    const dialog = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    expect(
      within(dialog).getByText(
        "Mail that says it’s from paypal.com but fails its sender checks → reported as spam",
      ),
    ).toBeVisible();
    expect(
      await within(dialog).findByText("Nothing here matches it yet. It acts on new mail."),
    ).toBeVisible();
    expect(within(dialog).queryByRole("checkbox")).toBeNull();
    expect(api.calls.some((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`)).toBe(
      false,
    );
  });
});
