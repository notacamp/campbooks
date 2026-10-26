import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { NOTION_CONNECTION } from "../../../../../test/demo/identity";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { NotionCard } from "./NotionCard";

const W = `/bff/api/v1/w/${DUARTE.id}`;
const START = `POST ${W}/connections/notion/authorizations`;
const IMPORT = `/w/${DUARTE.id}/pages?import=notion`;

function renderCard(
  connections: Parameters<typeof NotionCard>[0]["connections"],
  over: Partial<Parameters<typeof NotionCard>[0]> = {},
) {
  const leave = vi.fn();
  const onImport = vi.fn();
  renderWithProviders(
    <NotionCard
      workspaceId={DUARTE.id}
      connections={connections}
      returnTo={`/w/${DUARTE.id}/settings/mailboxes`}
      importHref={IMPORT}
      onImport={onImport}
      leave={leave}
      {...over}
    />,
  );
  return { leave, onImport };
}

describe("NotionCard", () => {
  it("says what connecting shares, and sends the browser to Notion", async () => {
    const mocked = mockApi({
      [START]: {
        status: 201,
        body: {
          authorize_url: "/auth/start/notion?ticket=abc",
          expires_at: "2026-10-25T09:10:00Z",
        },
      },
    });
    const { leave } = renderCard([]);

    expect(screen.getByRole("heading", { name: "Notion" })).toBeVisible();
    expect(
      screen.getByText(/Only the pages you pick in Notion’s own screen are shared/u),
    ).toBeVisible();
    expect(screen.getByText(/Disconnect any time/u)).toBeVisible();
    expect(screen.getByText(/Notion opens and asks which pages to share/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Connect Notion" }));

    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/notion?ticket=abc");
    });
    expect(mocked.calls[0]?.body).toEqual({ return_to: `/w/${DUARTE.id}/settings/mailboxes` });
    // The button stays off while the page is leaving.
    expect(screen.getByRole("button", { name: "Connect Notion" })).toBeDisabled();
  });

  it("says an instance without Notion has none", async () => {
    mockApi({ [START]: problem(409, "provider_not_configured") });
    renderCard([]);
    fireEvent.click(screen.getByRole("button", { name: "Connect Notion" }));
    expect(await screen.findByText("Notion isn’t available on this instance")).toBeVisible();
    expect(screen.queryByRole("button", { name: "Connect Notion" })).toBeNull();
  });

  it("says why a start was refused, and lets you try again", async () => {
    mockApi({ [START]: problem(429, "rate_limited") });
    renderCard([]);
    fireEvent.click(screen.getByRole("button", { name: "Connect Notion" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Too many tries");
    expect(screen.getByRole("button", { name: "Connect Notion" })).toBeEnabled();
  });

  it("waits for the list before offering anything", () => {
    renderCard();
    expect(screen.queryByRole("button", { name: "Connect Notion" })).toBeNull();
  });

  it("shows the connection with Import pages, which opens Pages", () => {
    const { onImport } = renderCard([NOTION_CONNECTION]);

    const list = screen.getByRole("list", { name: "Notion connections" });
    expect(within(list).getByText("Acme")).toBeVisible();
    expect(within(list).getByText("Notion · Connected by you")).toBeVisible();
    expect(within(list).getByText("Connected")).toBeVisible();
    const link = within(list).getByRole("link", { name: "Import pages from Acme" });
    expect(link).toHaveAttribute("href", IMPORT);
    fireEvent.click(link);
    expect(onImport).toHaveBeenCalled();
    // Connected already: no second Connect button.
    expect(screen.queryByRole("button", { name: "Connect Notion" })).toBeNull();
  });

  it("asks to reconnect when Notion needs the member again", async () => {
    const mocked = mockApi({
      [START]: {
        status: 201,
        body: {
          authorize_url: "/auth/start/notion?ticket=again",
          expires_at: "2026-10-25T09:10:00Z",
        },
      },
    });
    const { leave } = renderCard([{ ...NOTION_CONNECTION, health: "needs_reauth" }]);

    expect(screen.getByText("Needs you to sign in again")).toBeVisible();
    expect(screen.queryByRole("link", { name: /Import pages/u })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Reconnect Acme" }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/notion?ticket=again");
    });
    expect(mocked.calls[0]?.body).toEqual({
      return_to: `/w/${DUARTE.id}/settings/mailboxes`,
      connection: NOTION_CONNECTION.id,
    });
  });

  it("shows another member's connection without actions", () => {
    renderCard([{ ...NOTION_CONNECTION, granted_by_you: false, health: "needs_reauth" }]);
    expect(screen.getByText("Notion · Connected by another member")).toBeVisible();
    expect(screen.getByText("Needs the member who connected it to sign in again")).toBeVisible();
    expect(screen.queryByRole("button", { name: /Reconnect|Disconnect/u })).toBeNull();
    // Someone else's Notion: the card still offers you your own.
    expect(screen.getByRole("button", { name: "Connect Notion" })).toBeVisible();
  });

  it("describes a connection Notion is slowing or that is failing", () => {
    renderCard([
      { ...NOTION_CONNECTION, health: "throttled" },
      { ...NOTION_CONNECTION, id: "n2", health: "failing" },
    ]);
    expect(screen.getByText("Paused by Notion for a few minutes")).toBeVisible();
    expect(screen.getByText("Not syncing — we'll keep trying")).toBeVisible();
  });

  it("disconnects after asking, keeping the pages", async () => {
    const mocked = mockApi({
      [`DELETE ${W}/connections/${NOTION_CONNECTION.id}`]: { status: 204 },
    });
    renderCard([NOTION_CONNECTION]);

    fireEvent.click(screen.getByRole("button", { name: "Disconnect Acme" }));
    const group = await screen.findByRole("group");
    expect(within(group).getByText(/Its pages stay in Campbooks but stop syncing/u)).toBeVisible();
    fireEvent.click(within(group).getByRole("button", { name: "Disconnect" }));

    expect(await screen.findByText("Notion is disconnected")).toBeVisible();
    expect(mocked.calls[0]?.route).toBe(`DELETE ${W}/connections/${NOTION_CONNECTION.id}`);
  });

  it("can be cancelled, and says when a disconnect failed", async () => {
    mockApi({
      [`DELETE ${W}/connections/${NOTION_CONNECTION.id}`]: problem(500, "internal"),
    });
    renderCard([NOTION_CONNECTION]);
    fireEvent.click(screen.getByRole("button", { name: "Disconnect Acme" }));
    fireEvent.click(await screen.findByRole("button", { name: "Don’t disconnect" }));
    expect(screen.queryByRole("group")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Disconnect Acme" }));
    fireEvent.click(
      within(await screen.findByRole("group")).getByRole("button", { name: "Disconnect" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t disconnect Acme");
  });
});
