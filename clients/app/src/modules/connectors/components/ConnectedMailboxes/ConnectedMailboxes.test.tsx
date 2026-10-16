import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CONNECTION, SOFIA_CONNECTION } from "../../../../../test/demo/identity";
import { ANA, DUARTE, SOFIA } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { ConnectedMailboxes } from "./ConnectedMailboxes";

describe("ConnectedMailboxes", () => {
  it("names each connected mailbox, how it is doing, and where to manage them", async () => {
    mockApi({
      [`GET /bff/api/v1/w/${DUARTE.id}/members`]: { body: { members: [ANA, SOFIA] } },
    });
    renderWithProviders(
      <ConnectedMailboxes
        workspaceId={DUARTE.id}
        manage={{ href: "/w/ws/settings/mailboxes", onClick: vi.fn() }}
        connections={[
          CONNECTION,
          { ...CONNECTION, id: "c2", address: "bea@example.com", health: "needs_reauth" },
          {
            ...CONNECTION,
            id: "c3",
            address: "loja@example.net",
            provider: "zoho",
            health: "throttled",
          },
          { ...SOFIA_CONNECTION, health: "needs_reauth" },
        ]}
      />,
    );

    expect(screen.getByRole("heading", { name: "Your mail" })).toBeVisible();
    expect(screen.getByText("Syncing")).toBeVisible();
    expect(screen.getByText("Needs you to sign in again")).toBeVisible();
    expect(screen.getByText("Paused by Zoho Mail for a few minutes")).toBeVisible();
    expect(screen.getByText("bea@example.com")).toBeVisible();
    // A colleague's mailbox names who must sign in again, never "you".
    expect(await screen.findByText("Needs Sofia Pessoa to sign in again")).toBeVisible();
    expect(screen.getByRole("link", { name: "Manage mailboxes" })).toHaveAttribute(
      "href",
      "/w/ws/settings/mailboxes",
    );
  });
});
