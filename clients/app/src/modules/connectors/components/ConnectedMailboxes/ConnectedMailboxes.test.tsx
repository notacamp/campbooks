import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CONNECTION } from "../../../../../test/demo/identity";
import { renderWithProviders } from "../../../../../test/render";

import { ConnectedMailboxes } from "./ConnectedMailboxes";

describe("ConnectedMailboxes", () => {
  it("names each connected mailbox, how it is doing, and where to manage them", () => {
    renderWithProviders(
      <ConnectedMailboxes
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
        ]}
      />,
    );

    expect(screen.getByRole("heading", { name: "Your mail" })).toBeVisible();
    expect(screen.getByText("Syncing")).toBeVisible();
    expect(screen.getByText("Needs you to sign in again")).toBeVisible();
    expect(screen.getByText("Paused by Zoho Mail for a few minutes")).toBeVisible();
    expect(screen.getByText("bea@example.com")).toBeVisible();
    expect(screen.getByRole("link", { name: "Manage mailboxes" })).toHaveAttribute(
      "href",
      "/w/ws/settings/mailboxes",
    );
  });
});
