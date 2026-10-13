import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CONNECTION } from "../../../../../test/demo/identity";
import { renderWithProviders } from "../../../../../test/render";

import { ConnectedMailboxes } from "./ConnectedMailboxes";

describe("ConnectedMailboxes", () => {
  it("names each connected mailbox and how it is doing", () => {
    renderWithProviders(
      <ConnectedMailboxes
        connections={[
          CONNECTION,
          { ...CONNECTION, id: "c2", address: "bea@example.com", health: "needs_reauth" },
        ]}
      />,
    );

    expect(screen.getByRole("heading", { name: "Your mail" })).toBeVisible();
    expect(screen.getByText("Connected")).toBeVisible();
    expect(screen.getByText("Needs you to sign in again")).toBeVisible();
    expect(screen.getByText("bea@example.com")).toBeVisible();
  });
});
