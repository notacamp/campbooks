import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CONNECTION, NOTION_CONNECTION } from "../../../../../test/demo/identity";
import { DUARTE, SESSION } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";

import { ConnectionsPage, type ConnectionsPageProps } from "./ConnectionsPage";

const CONNECTIONS = `GET /bff/api/v1/w/${DUARTE.id}/connections`;

function api(connections: unknown[]) {
  return mockApi({
    "GET /bff/session": { body: SESSION },
    [CONNECTIONS]: { body: { connections } },
  });
}

function renderPage(props: Partial<ConnectionsPageProps> = {}) {
  renderWithProviders(<ConnectionsPage workspace={DUARTE} {...props} />);
}

describe("ConnectionsPage", () => {
  it("holds the Notion card, and no mailboxes", async () => {
    api([CONNECTION, NOTION_CONNECTION]);
    renderPage();

    expect(await screen.findByRole("heading", { level: 1, name: "Connections" })).toBeVisible();
    expect(screen.getByText(/Each connection is yours/u)).toBeVisible();
    const card = await screen.findByRole("list", { name: "Notion connections" });
    expect(within(card).getByText("Notion · Acme")).toBeVisible();
    expect(screen.queryByText("ana@example.com")).toBeNull();
  });

  it("offers Connect Notion when it is not connected", async () => {
    api([CONNECTION]);
    renderPage();
    expect(await screen.findByRole("button", { name: "Connect Notion" })).toBeVisible();
    expect(screen.getByText("Not connected")).toBeVisible();
  });

  it("points back to Mailboxes and Calendar", async () => {
    api([]);
    const onNavigate = vi.fn();
    renderPage({ onNavigate });

    const mail = await screen.findByRole("link", { name: "Mailboxes" });
    expect(mail).toHaveAttribute("href", `/w/${DUARTE.id}/settings/mailboxes`);
    fireEvent.click(mail);
    expect(onNavigate).toHaveBeenCalledWith(`/w/${DUARTE.id}/settings/mailboxes`);
    expect(screen.getByRole("link", { name: "Calendar" })).toHaveAttribute(
      "href",
      `/w/${DUARTE.id}/settings/calendar`,
    );
  });

  it("says Notion is connected when the browser comes back from it", async () => {
    api([NOTION_CONNECTION]);
    renderPage({ result: { kind: "connected", connection: NOTION_CONNECTION.id } });
    expect(await screen.findByText("Notion (Acme) is connected")).toBeVisible();
  });
});
