import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ADMIN_CONNECTIONS, ADMIN_DUARTE } from "../../../../../test/demo/admin";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { WorkspaceConnectionsAdmin } from "./WorkspaceConnectionsAdmin";

const CONNECTIONS = `GET /bff/admin/v1/connectors/workspaces/${ADMIN_DUARTE.id}/connections`;

describe("WorkspaceConnectionsAdmin", () => {
  it("shows each mailbox's health and how it syncs", async () => {
    mockApi({ [CONNECTIONS]: { body: { connections: ADMIN_CONNECTIONS } } });
    renderWithProviders(<WorkspaceConnectionsAdmin workspaceId={ADMIN_DUARTE.id} />);

    const healthy = within(
      (await screen.findByText("sales@example.com", { selector: "dt" })).parentElement ??
        document.body,
    ).getByRole("definition");
    expect(healthy).toHaveTextContent("Syncing");
    expect(healthy).toHaveTextContent("Google · Mail · Last synced");

    const broken = within(
      screen.getByText("billing@example.com", { selector: "dt" }).parentElement ?? document.body,
    ).getByRole("definition");
    expect(broken).toHaveTextContent("Needs the member to reconnect (since");
    expect(broken).toHaveTextContent("Paused · 3 failed runs in a row");
  });

  it("says when nothing is connected", async () => {
    mockApi({ [CONNECTIONS]: { body: { connections: [] } } });
    renderWithProviders(<WorkspaceConnectionsAdmin workspaceId={ADMIN_DUARTE.id} />);

    expect(await screen.findByText("No account connected in this workspace.")).toBeVisible();
  });
});
