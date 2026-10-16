import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";

import { useMailboxConsent, wayInFate } from "./consent";

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

/** What the hook says disconnecting connection `c1` does to sign-in. */
function Fate() {
  const consent = useMailboxConsent({ workspaceId: DUARTE.id, returnTo: HERE }, vi.fn());
  return <p>fate: {consent.wayInFate("c1") ?? "unknown"}</p>;
}

describe("the way-in sentence's source", () => {
  const items = [
    {
      id: "g",
      kind: "provider",
      provider: "google",
      email: "ana@example.com",
      linked_via: "sign_up",
      linked_at: "2026-10-01T09:00:00Z",
      last_used_at: null,
      mailboxes: 0,
      connections: [],
    },
    {
      id: "z",
      kind: "provider",
      provider: "zoho",
      email: "ana@example.net",
      linked_via: "connect",
      linked_at: "2026-10-02T09:00:00Z",
      last_used_at: null,
      mailboxes: 1,
      connections: ["c1"],
    },
  ];

  it("says nothing until the passkeys are known too", async () => {
    mockApi({
      [WAYS]: { body: { items, providers: [], mailbox_providers: [] } },
      "GET /bff/api/v1/identity/passkeys": { status: 503 },
    });
    renderWithProviders(<Fate />);
    expect(await screen.findByText("fate: unknown")).toBeVisible();
  });

  it("says it goes once both are known", async () => {
    mockApi({
      [WAYS]: { body: { items, providers: [], mailbox_providers: [] } },
      "GET /bff/api/v1/identity/passkeys": { body: { items: [] } },
    });
    renderWithProviders(<Fate />);
    expect(await screen.findByText("fate: removed")).toBeVisible();
  });
});

describe("wayInFate", () => {
  const way = (provider: string, linked_via: string, connections: string[]) => ({
    id: `${provider}-${linked_via}`,
    kind: "provider",
    provider,
    email: "ana@example.com",
    linked_via,
    linked_at: "2026-10-01T09:00:00Z",
    last_used_at: null,
    mailboxes: connections.length,
    connections,
  });
  const signedUp = way("google", "sign_up", []);
  const connected = way("zoho", "connect", ["c1"]);

  it("removes a way in a connect created with only this mailbox, unless it is the last", () => {
    expect(wayInFate([signedUp, connected], 0, "c1")).toBe("removed");
    expect(wayInFate([connected], 0, "c1")).toBe("unchanged");
    expect(wayInFate([connected], 1, "c1")).toBe("removed");
  });

  it("keeps one with another mailbox, one never recorded, or one not made by connecting", () => {
    const twice = { ...connected, mailboxes: 2, connections: ["c1", "c2"] };
    expect(wayInFate([signedUp, twice], 0, "c1")).toBe("unchanged");
    expect(wayInFate([signedUp, connected], 0, "c9")).toBe("unchanged");
    const linked = way("zoho", "link", ["c1"]);
    expect(wayInFate([signedUp, linked], 0, "c1")).toBe("unchanged");
  });
});
