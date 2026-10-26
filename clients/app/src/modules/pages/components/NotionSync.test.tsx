import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { NOTION_ROADMAP, notionLink } from "../../../../test/demo/pages";
import { renderWithProviders } from "../../../../test/render";

import { NotionSync } from "./NotionSync";

const NOW = new Date("2026-10-25T09:01:00Z");

function renderSync(
  link = notionLink("page-1", NOTION_ROADMAP),
  over: Partial<Parameters<typeof NotionSync>[0]> = {},
) {
  const onStop = vi.fn(() => Promise.resolve(true));
  const onShowConflict = vi.fn();
  renderWithProviders(
    <NotionSync
      link={link}
      canStop
      onStop={onStop}
      onShowConflict={onShowConflict}
      now={NOW}
      {...over}
    />,
  );
  return { onStop, onShowConflict };
}

describe("NotionSync", () => {
  it("says the page is synced, how long ago, and links to Notion", () => {
    renderSync();
    expect(screen.getByText(/^Synced with Notion · 1 min/u)).toBeVisible();
    const open = screen.getByRole("link", { name: "Open in Notion" });
    expect(open).toHaveAttribute("href", "https://www.notion.so/0192f1a00000700080000000000007a1");
    expect(open).toHaveAttribute("target", "_blank");
    expect(open).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("waits for the first sync, and says a paused one", () => {
    const { unmount } = renderWithProvidersFor(
      notionLink("p", NOTION_ROADMAP, { last_inbound_at: null }),
    );
    expect(screen.getByText("Linked to Notion, waiting for the first sync")).toBeVisible();
    unmount();
    renderSync(notionLink("p", NOTION_ROADMAP, { state: "paused" }));
    expect(screen.getByText("Sync with Notion is paused")).toBeVisible();
  });

  it("shows the changes to look at and jumps to the first", () => {
    const { onShowConflict } = renderSync(notionLink("p", NOTION_ROADMAP, { conflicts: 2 }));
    fireEvent.click(screen.getByRole("button", { name: "2 changes to look at" }));
    expect(onShowConflict).toHaveBeenCalled();
  });

  it("says one change in the singular", () => {
    renderSync(notionLink("p", NOTION_ROADMAP, { conflicts: 1 }));
    expect(screen.getByText("1 change to look at")).toBeVisible();
  });

  it("stops after asking, and stays asking when it could not", async () => {
    const onStop = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    renderSync(undefined, { onStop });

    fireEvent.click(screen.getByRole("button", { name: "Stop syncing" }));
    const group = await screen.findByRole("group", { name: /Stop keeping this page/u });
    fireEvent.click(within(group).getByRole("button", { name: "Stop syncing" }));
    await waitFor(() => {
      expect(onStop).toHaveBeenCalledTimes(1);
    });
    // Refused: still asking.
    expect(await screen.findByRole("group", { name: /Stop keeping this page/u })).toBeVisible();
    fireEvent.click(
      within(screen.getByRole("group", { name: /Stop keeping/u })).getByRole("button", {
        name: "Stop syncing",
      }),
    );
    await waitFor(() => {
      expect(screen.queryByRole("group", { name: /Stop keeping/u })).toBeNull();
    });
  });

  it("can be cancelled", async () => {
    const { onStop } = renderSync();
    fireEvent.click(screen.getByRole("button", { name: "Stop syncing" }));
    fireEvent.click(await screen.findByRole("button", { name: "Keep syncing" }));
    expect(screen.queryByRole("group", { name: /Stop keeping/u })).toBeNull();
    expect(onStop).not.toHaveBeenCalled();
  });

  it("says why a sync ended, and offers no stop", () => {
    renderSync(
      notionLink("p", NOTION_ROADMAP, {
        state: "stopped",
        stopped_reason: "not_shared",
        conflicts: 3,
      }),
    );
    expect(
      screen.getByText("Not synced: the page isn’t shared with Campbooks in Notion any more"),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Stop syncing" })).toBeNull();
    // Changes that wait mean nothing once it ended.
    expect(screen.queryByText(/changes to look at/u)).toBeNull();
    expect(screen.getByRole("link", { name: "Open in Notion" })).toBeVisible();
  });

  it("falls back to a general sentence for a reason it does not know", () => {
    renderSync(
      notionLink("p", NOTION_ROADMAP, {
        state: "stopped",
        stopped_reason: "something_new" as never,
      }),
    );
    expect(screen.getByText("Not synced with Notion any more")).toBeVisible();
  });

  it("offers no stop to someone who cannot write the page", () => {
    renderSync(undefined, { canStop: false });
    expect(screen.queryByRole("button", { name: "Stop syncing" })).toBeNull();
  });

  it("uses the real clock when none is given", () => {
    renderSync(undefined, { now: undefined });
    expect(screen.getByText(/^Synced with Notion · /u)).toBeVisible();
  });
});

function renderWithProvidersFor(link: ReturnType<typeof notionLink>) {
  return renderWithProviders(
    <NotionSync
      link={link}
      canStop
      onStop={() => Promise.resolve(true)}
      onShowConflict={vi.fn()}
      now={NOW}
    />,
  );
}
