import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { HELD_GRANT } from "../../../../../test/demo/identity";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { MailboxOffer } from "./MailboxOffer";

const ADOPT = `POST /bff/api/v1/w/${DUARTE.id}/identity/held-grants/${HELD_GRANT.id}/adopt`;
const DISCARD = `DELETE /bff/api/v1/identity/held-grants/${HELD_GRANT.id}`;

function renderOffer() {
  return renderWithProviders(
    <MailboxOffer grant={HELD_GRANT} workspaceId={DUARTE.id} workspaceName={DUARTE.name} />,
  );
}

describe("MailboxOffer", () => {
  it("connects the held mailbox to this workspace", async () => {
    const api = mockApi({ [ADOPT]: { body: { connection: "c1" } } });
    renderOffer();

    expect(screen.getByRole("heading", { name: "Connect ana@example.com" })).toBeVisible();
    expect(screen.getByText("Waiting until October 20")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toEqual([ADOPT]);
    });
  });

  it("explains a refusal in place", async () => {
    mockApi({ [ADOPT]: problem(409, "account_connected_by_another_member") });
    renderOffer();

    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Another member of this workspace connected ana@example.com.",
    );
  });

  it("forgets it only after confirming in place", async () => {
    const api = mockApi({ [DISCARD]: { status: 204 } });
    renderOffer();

    fireEvent.click(screen.getByRole("button", { name: "Don’t connect it" }));
    expect(screen.getByText(/Forget ana@example.com\?/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Keep it" }));
    expect(api.calls).toEqual([]);

    fireEvent.click(screen.getByRole("button", { name: "Don’t connect it" }));
    fireEvent.click(screen.getByRole("button", { name: "Forget it" }));
    await waitFor(() => {
      expect(api.calls.map((call) => call.route)).toEqual([DISCARD]);
    });
  });

  it("says when forgetting failed", async () => {
    mockApi({ [DISCARD]: problem(500, "internal") });
    renderOffer();

    fireEvent.click(screen.getByRole("button", { name: "Don’t connect it" }));
    fireEvent.click(screen.getByRole("button", { name: "Forget it" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t forget it.");
  });
});
