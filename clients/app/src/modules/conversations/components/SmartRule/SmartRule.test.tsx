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
import { ScoutChatProvider } from "~/lib/scout-chat";

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

  it("says when a smart rule already trusts the sender's images, and offers no retry", async () => {
    mockApi({
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: {
        status: 409,
        body: { code: "images_already_trusted", title: "Conflict", status: 409 },
      },
    });
    renderWithProviders(
      <SmartRuleConfirm
        workspaceId={DUARTE.id}
        draft={{ ...DRAFT, actions: [{ action: "show_images" }] }}
        onClose={vi.fn()}
      />,
    );
    const dialog = await screen.findByRole("dialog", { name: "A rule for mail like this" });
    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    expect(
      await within(dialog).findByText(/A smart rule already covers this sender’s images/u),
    ).toBeVisible();
    expect(within(dialog).queryByRole("button", { name: /Try again/u })).toBeNull();
    expect(within(dialog).getByRole("button", { name: /Save smart rule/u })).toBeDisabled();
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
        /You got 34 emails from CI \(builds@ci\.example\.com\) in the last 60 days and opened 2\. They come most days\. Keep them off Home and file them under Updates\?/u,
      ),
    ).toBeVisible();
    expect(within(note).getByText("1 of 1")).toBeVisible();

    // One style for the four buttons; the visor marks Something else….
    for (const name of [
      "File under Updates…",
      "Something else…",
      "Not now",
      "Don’t suggest this",
    ]) {
      expect(within(note).getByRole("button", { name })).toBeVisible();
    }
    expect(within(note).queryByRole("button", { name: "Set up a smart rule…" })).toBeNull();

    fireEvent.click(within(note).getByRole("button", { name: "File under Updates…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "A rule for mail from builds@ci.example.com",
    });
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

  it("asks Scout about the suggestion, where a chat is mounted", async () => {
    mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    const scout = vi.fn();
    renderWithProviders(
      <ScoutChatProvider open={scout}>
        <ScoutNoticed workspaceId={DUARTE.id} />
      </ScoutChatProvider>,
    );
    const note = await screen.findByRole("region", { name: "Scout noticed" });
    const ask = within(note).getByRole("button", { name: /Ask Scout/u });
    expect(ask).toHaveAttribute("aria-keyshortcuts", "Shift+Q");

    fireEvent.click(ask);
    expect(scout).toHaveBeenCalledWith(
      { type: "conversations.rule_notice", id: NOTICE.id },
      { from: ask },
    );
  });

  it("offers no Ask Scout where no chat is mounted", async () => {
    mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    const note = await screen.findByRole("region", { name: "Scout noticed" });

    expect(within(note).queryByRole("button", { name: /Ask Scout/u })).toBeNull();
  });

  it("asks what should happen first: the verb button starts on Scout's guess", async () => {
    const api = mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: created,
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    const note = await screen.findByRole("region", { name: "Scout noticed" });
    fireEvent.click(within(note).getByRole("button", { name: "File under Updates…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "A rule for mail from builds@ci.example.com",
    });
    const goals = within(dialog).getByRole("group", { name: "What should happen to CI’s mail?" });
    const guess = within(goals).getByRole("radio", { name: "Keep them off Home, under Updates" });
    expect(guess).toBeChecked();
    expect(guess).toHaveAccessibleDescription(/Scout’s guess/u);
    expect(within(goals).getByRole("radio", { name: "Highlight them" })).not.toBeChecked();
    expect(
      within(goals).getByRole("radio", { name: "Highlight them" }),
    ).toHaveAccessibleDescription(
      /I like these.*waits on Home under Worth a look until you have opened it once\. Nothing is pinned\./u,
    );
    expect(
      within(goals).getByRole("radio", { name: "File them under Newsletters" }),
    ).toBeInTheDocument();
    expect(within(goals).getByRole("radio", { name: "Mark them read" })).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    await screen.findByText("Smart rule saved");
    const made = api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`);
    expect(made?.body).toMatchObject({
      actions: NOTICE.actions,
      origin: "suggestion",
      notice_id: NOTICE.id,
    });
  });

  it("Something else… starts on nothing, and Highlight them answers the notice", async () => {
    const api = mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: created,
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    const note = await screen.findByRole("region", { name: "Scout noticed" });
    fireEvent.click(within(note).getByRole("button", { name: "Something else…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "A rule for mail from builds@ci.example.com",
    });
    for (const radio of within(dialog).getAllByRole("radio")) {
      expect(radio).not.toBeChecked();
    }
    // Nothing to save yet, and nothing to preview.
    expect(within(dialog).getByRole("button", { name: /Save smart rule/u })).toBeDisabled();
    expect(within(dialog).queryByText("Smart rule")).toBeNull();
    expect(api.calls.some((call) => call.route.endsWith("/rules/preview"))).toBe(false);

    fireEvent.click(within(dialog).getByRole("radio", { name: "Highlight them" }));
    expect(
      await within(dialog).findByText(
        "Mail from builds@ci.example.com → kept on Home until you open it",
      ),
    ).toBeVisible();
    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    await screen.findByText("Smart rule saved");
    const made = api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`);
    expect(made?.body).toMatchObject({
      conditions: NOTICE.conditions,
      actions: [{ action: "worth_a_look" }],
      notice_id: NOTICE.id,
    });
  });

  it("reads the member's own words through Scout and uses what it finds", async () => {
    const api = mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/${INBOX.id}/rules`]: created,
      [`POST ${W}/assistant/compile`]: {
        body: {
          outcome: "rules",
          text: "just archive them",
          proposals: [{ conditions: [], actions: [{ action: "archive" }], inboxes: [] }],
        },
      },
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    const note = await screen.findByRole("region", { name: "Scout noticed" });
    fireEvent.click(within(note).getByRole("button", { name: "Something else…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "A rule for mail from builds@ci.example.com",
    });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Or tell Scout in your words" }), {
      target: { value: "just archive them" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask Scout" }));
    expect(await within(dialog).findByRole("radio", { name: "As you said" })).toBeChecked();
    expect(
      await within(dialog).findByText("Mail from builds@ci.example.com → archived"),
    ).toBeVisible();
    fireEvent.click(within(dialog).getByRole("button", { name: /Save smart rule/u }));
    await screen.findByText("Smart rule saved");
    const made = api.calls.find((call) => call.route === `POST ${W}/inboxes/${INBOX.id}/rules`);
    expect(made?.body).toMatchObject({
      conditions: NOTICE.conditions,
      actions: [{ action: "archive" }],
      notice_id: NOTICE.id,
    });
    // The sender's display name (their own text) never reaches Scout's prompt.
    const asked = api.calls.find((call) => call.route === `POST ${W}/assistant/compile`);
    expect(asked?.body).toEqual({ text: "builds@ci.example.com: just archive them" });
  });

  it("says plainly when Scout cannot turn the words into a rule", async () => {
    mockApi({
      [`GET ${W}/rule-notices`]: { body: { notices: [NOTICE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`POST ${W}/assistant/compile`]: {
        body: { outcome: "unclear", text: "hmm", proposals: [] },
      },
    });
    renderWithProviders(<ScoutNoticed workspaceId={DUARTE.id} />);
    const note = await screen.findByRole("region", { name: "Scout noticed" });
    fireEvent.click(within(note).getByRole("button", { name: "Something else…" }));
    const dialog = await screen.findByRole("dialog", {
      name: "A rule for mail from builds@ci.example.com",
    });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Or tell Scout in your words" }), {
      target: { value: "hmm" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Ask Scout" }));
    expect(await within(dialog).findByText(/Scout couldn’t turn that into a rule/u)).toBeVisible();
    expect(within(dialog).queryByRole("radio", { name: "As you said" })).toBeNull();
    expect(within(dialog).getByRole("button", { name: /Save smart rule/u })).toBeDisabled();
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
