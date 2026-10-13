import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";

import { useMailboxConsent } from "./consent";

const WAYS = "GET /bff/api/v1/identity/ways-in";
const LINK = "POST /bff/api/v1/identity/links";
const HERE = `/w/${DUARTE.id}/settings/mailboxes`;

/** The hook behind two buttons, saying what it knows. */
function Door({ leave }: { leave: (url: string) => void }) {
  const consent = useMailboxConsent({ workspaceId: DUARTE.id, returnTo: HERE }, leave);
  return (
    <>
      <p>providers: {consent.providers?.join(",") ?? "…"}</p>
      <p>refusal: {consent.refusal ?? "none"}</p>
      <button
        type="button"
        onClick={() => {
          consent.start({ purpose: "connect", provider: "zoho", signIn: false });
        }}
      >
        connect
      </button>
      <button
        type="button"
        onClick={() => {
          consent.start({ purpose: "reconnect", connection: "c1" });
        }}
      >
        reconnect
      </button>
      <button type="button" onClick={consent.signInAgain}>
        again
      </button>
    </>
  );
}

const ways = {
  body: {
    items: [],
    providers: ["google", "microsoft", "zoho"],
    mailbox_providers: ["google", "zoho"],
  },
};

describe("useMailboxConsent", () => {
  it("offers the mailbox providers and connects through the front door, back to the page", async () => {
    const api = mockApi({
      [WAYS]: ways,
      [LINK]: { status: 201, body: { url: "/auth/start/zoho?link=t1", expires_at: "x" } },
    });
    const leave = vi.fn();
    renderWithProviders(<Door leave={leave} />);

    expect(await screen.findByText("providers: google,zoho")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "connect" }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/zoho?link=t1");
    });
    expect(api.calls.find((call) => call.route === LINK)?.body).toEqual({
      purpose: "connect",
      provider: "zoho",
      sign_in: false,
      workspace_id: DUARTE.id,
      return_to: HERE,
    });
  });

  it("reconnects a connection", async () => {
    const api = mockApi({
      [WAYS]: ways,
      [LINK]: { status: 201, body: { url: "/auth/start/google?link=t2", expires_at: "x" } },
    });
    renderWithProviders(<Door leave={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: "reconnect" }));
    await waitFor(() => {
      expect(api.calls.find((call) => call.route === LINK)?.body).toEqual({
        purpose: "reconnect",
        workspace_id: DUARTE.id,
        connection_id: "c1",
        return_to: HERE,
      });
    });
  });

  it("says why starting was refused, and signs in again back to the page", async () => {
    mockApi({ [WAYS]: ways, [LINK]: problem(403, "step_up_required") });
    const leave = vi.fn();
    renderWithProviders(<Door leave={leave} />);

    fireEvent.click(screen.getByRole("button", { name: "connect" }));
    expect(await screen.findByText("refusal: step_up")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "again" }));
    expect(leave).toHaveBeenCalledWith(
      `/bff/login?return_to=${encodeURIComponent(HERE)}&max_age=600`,
    );
  });

  it("explains a provider this instance cannot connect", async () => {
    mockApi({ [WAYS]: ways, [LINK]: problem(409, "mailbox_unsupported") });
    renderWithProviders(<Door leave={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "connect" }));
    expect(await screen.findByText("refusal: unsupported")).toBeVisible();
  });

  it("calls too many tries rate limited", async () => {
    mockApi({ [WAYS]: ways, [LINK]: problem(429, "rate_limited") });
    renderWithProviders(<Door leave={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "connect" }));
    expect(await screen.findByText("refusal: rate_limited")).toBeVisible();
  });
});
