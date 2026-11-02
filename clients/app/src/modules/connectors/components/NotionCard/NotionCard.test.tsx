import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { NOTION_CONNECTION } from "../../../../../test/demo/identity";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { NotionCard } from "./NotionCard";

const W = `/bff/api/v1/w/${DUARTE.id}`;
const START = `POST ${W}/connections/notion/authorizations`;
const IMPORT = `/w/${DUARTE.id}/pages?import=notion`;
const PAGES = `/w/${DUARTE.id}/pages`;
const CHECKED = {
  ...NOTION_CONNECTION,
  streams: [
    {
      capability: "pages" as const,
      paused: false,
      phase: "incremental" as const,
      last_synced_at: "2026-10-30T12:05:00Z",
    },
  ],
};

function renderCard(
  connections?: Parameters<typeof NotionCard>[0]["connections"],
  over: Partial<Parameters<typeof NotionCard>[0]> = {},
) {
  const leave = vi.fn();
  const onImport = vi.fn();
  const onOpenPages = vi.fn();
  renderWithProviders(
    <NotionCard
      workspaceId={DUARTE.id}
      connections={connections}
      returnTo={`/w/${DUARTE.id}/settings/connections`}
      importHref={IMPORT}
      onImport={onImport}
      pagesHref={PAGES}
      onOpenPages={onOpenPages}
      now={new Date("2026-10-30T12:10:00Z")}
      leave={leave}
      {...over}
    />,
  );
  return { leave, onImport, onOpenPages };
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
    expect(mocked.calls[0]?.body).toEqual({ return_to: `/w/${DUARTE.id}/settings/connections` });
    // The button stays off while the page is leaving.
    expect(screen.getByRole("button", { name: "Connect Notion" })).toBeDisabled();
  });

  it("says an instance without Notion has none", async () => {
    mockApi({ [START]: problem(409, "provider_not_configured") });
    renderCard([]);
    fireEvent.click(screen.getByRole("button", { name: "Connect Notion" }));
    expect(await screen.findByText("Notion isn’t available on this instance.")).toBeVisible();
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

  it("shows the connection, what it brings in and shares, and its actions", () => {
    const { onImport, onOpenPages } = renderCard([NOTION_CONNECTION]);

    const list = screen.getByRole("list", { name: "Notion connections" });
    expect(within(list).getByText("Notion · Acme")).toBeVisible();
    expect(within(list).getByText("Connected by you")).toBeVisible();
    expect(within(list).getByText("Connected")).toBeVisible();
    expect(within(list).getByText("What it brings in")).toBeVisible();
    expect(
      within(list).getByText("Pages you imported, in your Private place in Pages"),
    ).toBeVisible();
    expect(within(list).getByText(/checks Notion for changes every few minutes/u)).toBeVisible();
    expect(
      within(list).getByText(/Only the pages you picked in Notion’s own screen/u),
    ).toBeVisible();
    const link = within(list).getByRole("link", { name: "Import pages from Acme" });
    expect(link).toHaveAttribute("href", IMPORT);
    fireEvent.click(link);
    expect(onImport).toHaveBeenCalled();
    const open = within(list).getByRole("link", { name: "Open in Pages" });
    expect(open).toHaveAttribute("href", PAGES);
    fireEvent.click(open);
    expect(onOpenPages).toHaveBeenCalled();
    // Connected already: no second Connect button.
    expect(screen.queryByRole("button", { name: "Connect Notion" })).toBeNull();
    // The actions end with Disconnect.
    const actions = [...list.querySelectorAll("a, button")].map((control) => control.textContent);
    expect(actions.at(-1)).toContain("Disconnect");
  });

  it("says when Notion was last checked, only when it is healthy and has been", () => {
    renderCard([CHECKED]);
    expect(screen.getByText("Connected · checked Notion 5 min. ago")).toBeVisible();
    expect(document.body.textContent).not.toMatch(/\blive\b|real time/iu);
  });

  it("says nothing of a check that never happened or that failed", () => {
    renderCard([
      NOTION_CONNECTION,
      { ...CHECKED, id: "n2", health: "failing" },
      { ...CHECKED, id: "n3", health: "needs_reauth" },
    ]);
    expect(screen.queryByText(/checked Notion/u)).toBeNull();
  });

  it("says Not connected before connecting", () => {
    renderCard([]);
    expect(screen.getByText("Not connected")).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect Notion" })).toBeVisible();
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
      return_to: `/w/${DUARTE.id}/settings/connections`,
      connection: NOTION_CONNECTION.id,
    });
  });

  it("shows another member's connection without actions", () => {
    renderCard([{ ...NOTION_CONNECTION, granted_by_you: false, health: "needs_reauth" }]);
    expect(screen.getByText("Connected by another member")).toBeVisible();
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

  it("asks what stops and what stays, then disconnects, keeping the pages", async () => {
    const mocked = mockApi({
      [`DELETE ${W}/connections/${NOTION_CONNECTION.id}`]: { status: 204 },
    });
    renderCard([NOTION_CONNECTION]);

    fireEvent.click(screen.getByRole("button", { name: "Disconnect Acme" }));
    const dialog = await screen.findByRole("dialog", { name: "Disconnect Notion (Acme)?" });
    expect(within(dialog).getByText("What stops")).toBeVisible();
    expect(within(dialog).getByText("Changes in Notion no longer come in")).toBeVisible();
    expect(within(dialog).getByText("Your edits no longer go back to Notion")).toBeVisible();
    expect(within(dialog).getByText("Campbooks forgets its access to your Notion")).toBeVisible();
    expect(within(dialog).getByText("What stays")).toBeVisible();
    expect(
      within(dialog).getByText("The pages you imported, in Pages, as they are now"),
    ).toBeVisible();
    expect(
      within(dialog).getByText("Everything in Notion: nothing is deleted there"),
    ).toBeVisible();
    expect(
      within(dialog).getByText("You can connect Notion again at any time from here."),
    ).toBeVisible();
    fireEvent.click(within(dialog).getByRole("button", { name: /^Disconnect/u }));

    expect(await screen.findByText("Notion is disconnected")).toBeVisible();
    expect(mocked.calls[0]?.route).toBe(`DELETE ${W}/connections/${NOTION_CONNECTION.id}`);
    expect(mocked.calls).toHaveLength(1);
  });

  it("can be kept connected, and says when a disconnect failed", async () => {
    mockApi({
      [`DELETE ${W}/connections/${NOTION_CONNECTION.id}`]: problem(500, "internal"),
    });
    renderCard([NOTION_CONNECTION]);
    fireEvent.click(screen.getByRole("button", { name: "Disconnect Acme" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Keep connected/u }));
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Disconnect Acme" }));
    fireEvent.click(
      within(await screen.findByRole("dialog")).getByRole("button", { name: /^Disconnect/u }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t disconnect Acme");
  });
});
