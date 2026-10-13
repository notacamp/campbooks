import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { HELD_GRANT } from "../../../../../test/demo/identity";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { MailboxStep } from "./MailboxStep";

const HELD = "GET /bff/api/v1/identity/held-grants";
const ADOPT = `POST /bff/api/v1/w/${DUARTE.id}/identity/held-grants/${HELD_GRANT.id}/adopt`;

function renderStep() {
  const onContinue = vi.fn();
  renderWithProviders(
    <MailboxStep workspace={DUARTE} onContinue={onContinue}>
      <p>Duarte & Pessoa is ready</p>
    </MailboxStep>,
  );
  return onContinue;
}

describe("MailboxStep", () => {
  it("offers the held Gmail for the new workspace and connects it with Enter", async () => {
    const api = mockApi({
      [HELD]: { body: { items: [HELD_GRANT] } },
      [ADOPT]: { body: { connection: "c1" } },
    });
    const onContinue = renderStep();

    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: "Your Gmail ana@example.com will be connected to this workspace",
      }),
    ).toBeVisible();
    expect(screen.getByText("Duarte & Pessoa is ready")).toBeVisible();
    expect(screen.getByText(/Not now keeps it waiting on Home until October 20/u)).toBeVisible();
    const connect = screen.getByRole("button", { name: /^Connect/u });
    await waitFor(() => {
      expect(connect).toHaveFocus();
    });

    fireEvent.submit(connect);
    await waitFor(() => {
      expect(onContinue).toHaveBeenCalledOnce();
    });
    expect(api.calls.map((call) => call.route)).toContain(ADOPT);
  });

  it("goes on without connecting with Not now or Escape", async () => {
    const api = mockApi({ [HELD]: { body: { items: [HELD_GRANT] } } });
    const onContinue = renderStep();

    fireEvent.click(await screen.findByRole("button", { name: /Not now/u }));
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(onContinue).toHaveBeenCalledTimes(2);
    expect(api.calls.map((call) => call.route)).toEqual([HELD]);
  });

  it("shows the flow's own view when nothing is held, or the list can't be read", async () => {
    mockApi({ [HELD]: { body: { items: [] } } });
    renderStep();
    expect(await screen.findByText("Duarte & Pessoa is ready")).toBeVisible();
  });

  it.each([
    [
      problem(409, "account_connected_by_another_member"),
      "Someone else here already connected it.",
    ],
    [problem(409, "mailbox_unsupported"), "Campbooks can’t read this mailbox yet."],
    [problem(404, "held_grant_not_found"), "That mailbox isn’t waiting any more."],
  ])("explains a refusal and only offers to continue", async (answer, says) => {
    mockApi({ [HELD]: { body: { items: [HELD_GRANT] } }, [ADOPT]: answer });
    const onContinue = renderStep();

    fireEvent.click(await screen.findByRole("button", { name: /^Connect/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent(says);
    fireEvent.click(screen.getByRole("button", { name: /Continue to Home/u }));
    expect(onContinue).toHaveBeenCalledOnce();
  });

  it("lets a failure be retried", async () => {
    mockApi({ [HELD]: { body: { items: [HELD_GRANT] } }, [ADOPT]: problem(500, "internal") });
    renderStep();

    fireEvent.click(await screen.findByRole("button", { name: /^Connect/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent("It didn’t connect.");
    expect(screen.getByRole("button", { name: /^Connect/u })).toBeEnabled();
  });
});
