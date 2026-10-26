import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { W } from "../../../../test/demo/conversations";
import {
  NOTION_NOTES,
  NOTION_OKRS,
  NOTION_PAGES,
  NOTION_ROADMAP,
} from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";
import type { NotionSource, UseNotionSource } from "../notion-source";

import { ImportFromNotion, MAX_IMPORT } from "./ImportFromNotion";

const CONNECTION = { id: "c1", name: "Acme", needsReauth: false };

function source(
  over: Partial<NotionSource["list"]> = {},
  connections = [CONNECTION],
): UseNotionSource {
  return () => ({
    connections,
    list: {
      status: "ready",
      pages: NOTION_PAGES,
      truncated: false,
      error: undefined,
      retry: vi.fn(),
      ...over,
    },
  });
}

function renderImport(useSource: UseNotionSource, extra: Record<string, unknown> = {}) {
  const mocked = mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/pages`]: { body: { pages: [] } },
    [`GET ${W}/pages/notion/links`]: { body: { links: [] } },
    [`POST ${W}/pages/notion/imports`]: {
      status: 201,
      body: { pages: [{ page: "new-1", notion_page: NOTION_ROADMAP }], skipped: 0 },
    },
    ...extra,
  });
  const onClose = vi.fn();
  const onImported = vi.fn();
  renderWithProviders(
    <ImportFromNotion
      workspaceId={DUARTE.id}
      open
      onClose={onClose}
      useSource={useSource}
      settingsHref="/w/x/settings/mailboxes"
      onImported={onImported}
    />,
  );
  return { mocked, onClose, onImported };
}

describe("ImportFromNotion", () => {
  it("lists the shared pages with sub-page counts, and says what is kept in Notion", async () => {
    renderImport(source());

    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(
      within(dialog).getByText(/These are the pages you shared with Campbooks in Notion/u),
    ).toBeVisible();
    await waitFor(() => {
      expect(within(dialog).getByText(/^Connected as .*Acme/u)).toBeVisible();
    });
    expect(within(dialog).getByText("2 sub-pages")).toBeVisible();
    expect(within(dialog).getByLabelText("Q4 OKRs")).toBeVisible();
    expect(within(dialog).getByText("Already synced")).toBeVisible();
    expect(within(dialog).getByLabelText("Hiring plan")).toBeDisabled();
    expect(within(dialog).getByText(/come across as a “Kept in Notion” box/u)).toBeVisible();
    expect(within(dialog).getByRole("button", { name: "Import pages" })).toBeDisabled();
  });

  it("imports the ticked pages, sub-pages along, and opens the first", async () => {
    const { mocked, onClose, onImported } = renderImport(source());
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });

    fireEvent.click(within(dialog).getByLabelText("Product roadmap"));
    // The sub-pages came along: ticked, and not theirs to untick.
    expect(within(dialog).getByLabelText("Q4 OKRs")).toBeChecked();
    expect(within(dialog).getByLabelText("Q4 OKRs")).toBeDisabled();
    fireEvent.click(within(dialog).getByLabelText("Meeting notes"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Import 4 pages" }));

    await waitFor(() => {
      expect(onImported).toHaveBeenCalledWith("new-1");
    });
    expect(onClose).toHaveBeenCalled();
    expect(mocked.calls.find((c) => c.route === `POST ${W}/pages/notion/imports`)?.body).toEqual({
      connection: "c1",
      pages: [
        { notion_page: NOTION_ROADMAP, title: "Product roadmap" },
        { notion_page: NOTION_NOTES, title: "Meeting notes" },
      ],
    });
  });

  it("imports one page in the singular", async () => {
    renderImport(source());
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByLabelText("Q4 OKRs"));
    expect(within(dialog).getByRole("button", { name: "Import 1 page" })).toBeEnabled();
    expect(NOTION_OKRS).toBeTruthy();
  });

  it("says when the import failed, and keeps the choice", async () => {
    renderImport(source(), {
      [`POST ${W}/pages/notion/imports`]: { status: 500, body: { status: 500, title: "x" } },
    });
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByLabelText("Meeting notes"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Import 1 page" }));
    expect(await within(dialog).findByText("Couldn’t import. Nothing was changed.")).toBeVisible();
    expect(within(dialog).getByLabelText("Meeting notes")).toBeChecked();
  });

  it("refuses more pages than the server takes", async () => {
    const many = Array.from({ length: MAX_IMPORT + 1 }, (_, n) => ({
      notion_page: `p${String(n)}`,
      title: `Page ${String(n)}`,
      edited_at: "2026-10-25T09:00:00Z",
      parent: null,
      sub_pages: 0,
      linked: false,
    }));
    renderImport(source({ pages: many }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    for (const box of within(dialog).getAllByRole("checkbox")) {
      fireEvent.click(box);
    }
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      "Pick at most 100 pages at a time.",
    );
    expect(within(dialog).getByRole("button", { name: /^Import \d+ pages/u })).toBeDisabled();
  });

  it("says how a listing failed, and tries again", async () => {
    const retry = vi.fn();
    renderImport(source({ status: "error", pages: [], error: "unavailable", retry }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByText("Notion couldn’t be reached")).toBeVisible();
    fireEvent.click(within(dialog).getByRole("button", { name: "Try again" }));
    expect(retry).toHaveBeenCalled();
  });

  it("sends a connection that needs Notion again to Settings", async () => {
    renderImport(source({ status: "error", pages: [], error: "needs_reauth" }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByText("Notion needs you to connect again")).toBeVisible();
    expect(within(dialog).getByRole("link", { name: "Open Settings" })).toHaveAttribute(
      "href",
      "/w/x/settings/mailboxes",
    );
  });

  it("does the same for a connection already marked as needing the member", async () => {
    renderImport(source({}, [{ ...CONNECTION, needsReauth: true }]));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByText("Notion needs you to connect again")).toBeVisible();
    expect(within(dialog).getByRole("button", { name: "Import pages" })).toBeDisabled();
  });

  it("asks to connect Notion first when there is no connection", async () => {
    renderImport(source({ status: "idle", pages: [] }, []));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByText("Connect Notion first")).toBeVisible();
  });

  it("says nothing is shared yet, and that the list is cut when it is", async () => {
    const { unmount } = renderImport(source({ pages: [] }));
    expect(await screen.findByText("No pages shared yet")).toBeVisible();
    unmount();
    renderImport(source({ truncated: true }));
    expect(await screen.findByText(/Notion lists more pages than are shown here/u)).toBeVisible();
  });

  it("shows a skeleton while the pages load", async () => {
    renderImport(source({ status: "loading", pages: [] }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    expect(within(dialog).getByRole("status")).toBeVisible();
  });

  it("lets the member choose among several Notion accounts", async () => {
    renderImport(source({}, [CONNECTION, { id: "c2", name: "Home", needsReauth: false }]));
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    const account = within(dialog).getByLabelText("Notion account");
    fireEvent.change(account, { target: { value: "c2" } });
    expect(account).toHaveValue("c2");
  });

  it("closes with nothing ticked kept", async () => {
    const { onClose } = renderImport(source());
    const dialog = await screen.findByRole("dialog", { name: "Import from Notion" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalled();
  });
});
