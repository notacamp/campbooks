import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { HOUSEHOLD, SESSION } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { AcceptInvitation } from "./AcceptInvitation";

const ACCEPT = "POST /bff/api/v1/workspaces/invitations/accept";

function renderPage(token?: string) {
  const handlers = { onOpen: vi.fn(), onHome: vi.fn(), onSignOut: vi.fn() };
  renderWithProviders(<AcceptInvitation token={token} {...handlers} />);
  return handlers;
}

describe("AcceptInvitation", () => {
  it("accepts once and offers to open the workspace", async () => {
    const api = mockApi({ [ACCEPT]: { body: HOUSEHOLD }, "GET /bff/session": { body: SESSION } });
    const { onOpen } = renderPage("t0ken");

    expect(
      await screen.findByRole("heading", { level: 1, name: "You’re in Household" }),
    ).toBeVisible();
    expect(screen.getByText("You joined as Member.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Open Household" }));

    expect(onOpen).toHaveBeenCalledWith(HOUSEHOLD);
    expect(api.calls.filter((call) => call.route === ACCEPT)).toEqual([
      { route: ACCEPT, body: { token: "t0ken" } },
    ]);
  });

  it.each([
    ["invitation_used", 409, "This invitation was already used"],
    ["invitation_expired", 409, "This invitation has expired"],
    ["already_a_member", 409, "You’re already a member"],
    ["not_found", 404, "This invitation link doesn’t work"],
    ["internal", 500, "Couldn’t accept the invitation"],
  ])("explains %s", async (code, status, title) => {
    mockApi({ [ACCEPT]: problem(status, code), "GET /bff/session": { body: SESSION } });
    const { onHome } = renderPage("t0ken");

    expect(await screen.findByRole("heading", { level: 1, name: title })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Go to my workspaces" }));
    expect(onHome).toHaveBeenCalled();
  });

  it("offers to sign out when the invitation is for someone else", async () => {
    mockApi({
      [ACCEPT]: problem(403, "invitation_email_mismatch"),
      "GET /bff/session": { body: SESSION },
    });
    const { onSignOut } = renderPage("t0ken");

    expect(
      await screen.findByRole("heading", { name: "This invitation is for another email address" }),
    ).toBeVisible();
    await waitFor(() => {
      expect(screen.getByText(/You’re signed in as ana@example.com/u)).toBeVisible();
    });
    fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
    expect(onSignOut).toHaveBeenCalled();
  });

  it("retries after a failure", async () => {
    let attempts = 0;
    mockApi({
      [ACCEPT]: () => {
        attempts += 1;
        return attempts === 1 ? problem(503, "unavailable") : { body: HOUSEHOLD };
      },
      "GET /bff/session": { body: SESSION },
    });
    renderPage("t0ken");

    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("heading", { name: "You’re in Household" })).toBeVisible();
  });

  it("says a link without a token doesn't work, without calling the API", async () => {
    const api = mockApi({ "GET /bff/session": { body: SESSION } });
    renderPage();

    expect(
      await screen.findByRole("heading", { name: "This invitation link doesn’t work" }),
    ).toBeVisible();
    expect(api.calls.some((call) => call.route === ACCEPT)).toBe(false);
  });
});
