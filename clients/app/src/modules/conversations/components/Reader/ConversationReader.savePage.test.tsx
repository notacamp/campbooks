// Save to page in the reader (campbooks-cloud#364): in the More menu and on ⇧P,
// only while the server offers mail and Scout in pages.
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { INBOX, Q4, Q4_BODY, Q4_DETAIL, W } from "../../../../../test/demo/conversations";
import { CLIENT_ONBOARDING } from "../../../../../test/demo/pages";
import { forRequests, saveToPageServer } from "../../../../../test/demo/save-to-page";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import { useInboxActions } from "../InboxScreen/use-inbox-actions";

import { ConversationReader } from "./ConversationReader";

function Reader() {
  const actions = useInboxActions(DUARTE.id);
  return (
    <ConversationReader
      workspaceId={DUARTE.id}
      conversationId={Q4.id}
      stream="primary"
      actions={actions}
      onBack={vi.fn()}
      onPrevious={vi.fn()}
      onNext={vi.fn()}
      onMove={vi.fn()}
    />
  );
}

const NEWEST = Q4_DETAIL.messages[2]?.id ?? "";

function setup(options: { off?: boolean; empty?: boolean } = {}) {
  const pages = saveToPageServer({ off: options.off === true, empty: options.empty === true });
  const mocked = mockApi({
    ...forRequests(pages.routes),
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
    [`GET ${W}/conversations/${Q4.id}/messages/${NEWEST}/body`]: { body: Q4_BODY },
    [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
  });
  const root = createRootRoute({ component: Reader });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  return { pages, mocked, ...renderWithProviders(<RouterProvider router={router} />) };
}

describe("Save to page in the reader", () => {
  it("is among the reader's actions with its key, and opens the dialog for this thread", async () => {
    setup();
    await screen.findByRole("heading", { name: Q4.subject });
    const entry = await screen.findByRole("button", { name: /Save to page/u });
    expect(entry).toHaveAttribute("aria-keyshortcuts", "Shift+P");

    fireEvent.click(entry);
    expect(
      await screen.findByRole("dialog", { name: `Save “${Q4.subject}” to a page` }),
    ).toBeVisible();
    expect(await screen.findByRole("option", { name: CLIENT_ONBOARDING.title })).toBeVisible();
  });

  it("opens on ⇧P", async () => {
    setup();
    await screen.findByRole("heading", { name: Q4.subject });
    await screen.findByRole("button", { name: /Save to page/u });
    fireEvent.keyDown(document.body, { key: "P", shiftKey: true });
    expect(
      await screen.findByRole("dialog", { name: `Save “${Q4.subject}” to a page` }),
    ).toBeVisible();
  });

  it("has no entry and no key while the server does not offer it", async () => {
    setup({ off: true });
    await screen.findByRole("heading", { name: Q4.subject });
    await screen.findByRole("button", { name: /Make it a task/u });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: /Save to page/u })).not.toBeInTheDocument();
    });
    fireEvent.keyDown(document.body, { key: "P", shiftKey: true });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("is offered in a workspace with no pages yet, and asks nothing about a page's sharing", async () => {
    const { mocked } = setup({ empty: true });
    await screen.findByRole("heading", { name: Q4.subject });
    fireEvent.click(await screen.findByRole("button", { name: /Save to page/u }));
    expect(
      await screen.findByRole("dialog", { name: `Save “${Q4.subject}” to a page` }),
    ).toBeVisible();
    expect(await screen.findByRole("button", { name: "New page" })).toBeVisible();
    expect(mocked.calls.some((call) => call.route.includes("/sharing"))).toBe(false);
  });
});
