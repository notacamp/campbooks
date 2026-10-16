import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { TeachScout } from "./TeachScout";

const WS = "ws-1";
const W = `/bff/api/v1/w/${WS}`;
const SENTENCE = "anything from my accountant always matters";
const INBOX = { id: "inbox-own", address: "ines@ateliersousa.example", shared: false };
const ACCOUNTANT = {
  outcome: "rules",
  unsupported: null,
  text: SENTENCE,
  proposals: [
    {
      conditions: [],
      actions: [{ action: "surface", value: "always" }],
      inboxes: [INBOX],
      who: {
        phrase: "my accountant",
        matched: false,
        candidates: [
          { address: "rita@silva-contas.example", name: "Rita Gomes", received: 41, replied: 23 },
        ],
      },
    },
  ],
};
const PREVIEW = {
  matching: 41,
  would_change: 41,
  samples: [],
  skipped_suspicious: 0,
  skipped_member: 0,
  overrides: [],
};

function teach(text = SENTENCE) {
  const field = screen.getByRole("textbox", { name: "What should Scout do?" });
  fireEvent.change(field, { target: { value: text } });
  fireEvent.click(screen.getByRole("button", { name: /Teach/u }));
}

describe("TeachScout", () => {
  it("asks who someone is, shows the rule and what it matches, and saves it as the member's words", async () => {
    const api = mockApi({
      [`POST ${W}/assistant/compile`]: { body: ACCOUNTANT },
      [`POST ${W}/inboxes/inbox-own/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/inbox-own/rules`]: {
        status: 201,
        body: { rule: { id: "rule-1" }, acted_without_asking: false },
      },
    });
    renderWithProviders(<TeachScout workspaceId={WS} />);

    teach();
    expect(await screen.findByText("Who do you mean by “my accountant”?")).toBeVisible();
    expect(api.calls[0]?.body).toEqual({ text: SENTENCE });
    fireEvent.click(screen.getByRole("button", { name: "Rita Gomes · rita@silva-contas.example" }));

    expect(
      await screen.findByText(
        "Mail from Rita Gomes (rita@silva-contas.example) always stays in your inbox and never gets filed away.",
      ),
    ).toBeVisible();
    expect(await screen.findByText("Matches 41 emails")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Save rule/u }));

    expect(await screen.findByText("Rule saved")).toBeVisible();
    const created = api.calls.find((call) => call.route === `POST ${W}/inboxes/inbox-own/rules`);
    expect(created?.body).toMatchObject({
      conditions: [{ field: "sender_address", value: "rita@silva-contas.example" }],
      actions: [{ action: "surface", value: "always" }],
      origin: "instruction",
      origin_text: SENTENCE,
      state: "active",
    });
  });

  it("says plainly what rules can't do", async () => {
    mockApi({
      [`POST ${W}/assistant/compile`]: {
        body: {
          outcome: "unsupported",
          unsupported: "sends_mail",
          text: "forward invoices",
          proposals: [],
        },
      },
    });
    renderWithProviders(<TeachScout workspaceId={WS} />);

    teach("forward invoices");
    expect(
      await screen.findByText(
        "Scout can’t send, forward or reply to mail. Rules only sort, label, archive, and show or hide it.",
      ),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: /Save rule/u })).toBeNull();
  });

  it("offers the rule builder when no language model may read sentences here", async () => {
    mockApi({ [`POST ${W}/assistant/compile`]: problem(409, "compile_unavailable") });
    renderWithProviders(<TeachScout workspaceId={WS} />);

    teach();
    expect(
      await screen.findByText(
        "This server has no language model for reading sentences, so build the rule here.",
      ),
    ).toBeVisible();
    expect(screen.getByRole("combobox", { name: /Which mail/u })).toBeVisible();
  });

  it("explains a refused sentence", async () => {
    mockApi({ [`POST ${W}/assistant/compile`]: problem(400, "instruction_pasted_mail") });
    renderWithProviders(<TeachScout workspaceId={WS} />);

    teach("From: Ana");
    expect(await screen.findByText(/looks pasted from an email/u)).toBeVisible();
  });

  it("keeps the focus in the sentence, announces the rule, saves with ⌘↵ and retries only the inbox that failed", async () => {
    const other = { id: "inbox-two", address: "ines@sousa-pessoal.example", shared: false };
    let tries = 0;
    const api = mockApi({
      [`POST ${W}/assistant/compile`]: {
        body: {
          outcome: "rules",
          unsupported: null,
          text: "rita always matters",
          proposals: [
            {
              conditions: [{ field: "sender_address", value: "rita@silva-contas.example" }],
              actions: [{ action: "surface", value: "always" }],
              inboxes: [INBOX, other],
              who: null,
            },
          ],
        },
      },
      [`POST ${W}/inboxes/inbox-own/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/inbox-two/rules/preview`]: { body: PREVIEW },
      [`POST ${W}/inboxes/inbox-own/rules`]: {
        status: 201,
        body: { rule: { id: "rule-1" }, acted_without_asking: false },
      },
      [`POST ${W}/inboxes/inbox-two/rules`]: () => {
        tries += 1;
        return tries === 1
          ? problem(503, "unavailable")
          : { status: 201, body: { rule: { id: "rule-2" }, acted_without_asking: false } };
      },
      [`DELETE ${W}/rules/rule-1`]: problem(503, "unavailable"),
      [`DELETE ${W}/rules/rule-2`]: problem(503, "unavailable"),
    });
    renderWithProviders(<TeachScout workspaceId={WS} />);

    const field = screen.getByRole("textbox", { name: "What should Scout do?" });
    field.focus();
    fireEvent.change(field, { target: { value: "rita always matters" } });
    fireEvent.submit(field);
    expect(
      await screen.findByText(
        /^Here’s the rule I understood: Mail from rita@silva-contas\.example/u,
      ),
    ).toBeInTheDocument();
    expect(field).toHaveFocus();

    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(
      await screen.findByText(
        "Saved in 1 of 2 inboxes, then Scout stopped. Try again saves it in the rest.",
      ),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Try again/u }));
    expect(await screen.findByText("2 rules saved")).toBeVisible();
    const routes = api.calls
      .filter((call) => call.route.startsWith("POST") && call.route.endsWith("/rules"))
      .map((call) => call.route);
    expect(routes).toEqual([
      `POST ${W}/inboxes/inbox-own/rules`,
      `POST ${W}/inboxes/inbox-two/rules`,
      `POST ${W}/inboxes/inbox-two/rules`,
    ]);

    // An Undo that fails says so.
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    expect(
      await screen.findByText(
        "Scout couldn’t undo that. Your rules are in Scout’s memory, where you can remove them.",
      ),
    ).toBeVisible();
  });

  it("doesn't blame a missing model when reading the sentence just failed", async () => {
    mockApi({ [`POST ${W}/assistant/compile`]: problem(502, "upstream") });
    renderWithProviders(<TeachScout workspaceId={WS} />);

    teach();
    fireEvent.click(await screen.findByRole("button", { name: /Show the rule/u }));
    await waitFor(() => {
      expect(
        screen.getAllByText("Scout couldn’t read your sentence, so build the rule here.")[0],
      ).toBeVisible();
    });
    expect(screen.queryByText(/This server has no language model/u)).toBeNull();
  });
});
