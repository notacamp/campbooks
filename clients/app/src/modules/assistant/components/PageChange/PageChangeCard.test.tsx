import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  KICKOFF_NOTES,
  LONG_NOTES,
  MARTA_CHAT,
  NEW_PAGE_PROPOSAL,
  pageProposal,
} from "../../../../../test/demo/assistant";
import { MARTA, PEDRO } from "../../../../../test/demo/attention";
import { W } from "../../../../../test/demo/conversations";
import { CLIENT_ONBOARDING } from "../../../../../test/demo/pages";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import type { Proposal } from "../../api/proposals";

import { PageChangeCard } from "./PageChangeCard";

const PAGE = CLIENT_ONBOARDING.id;
const APPEND = `POST ${W}/pages/${PAGE}/append`;
const UNAPPEND = `POST ${W}/pages/${PAGE}/unappend`;
const CREATE = `POST ${W}/pages`;
const NEW_ID = "0192f1a0-0000-7000-8000-0000000009ff";
const SETTLE = (proposal: Proposal) =>
  `POST ${W}/assistant/chats/${MARTA_CHAT.id}/proposals/${proposal.id}`;

const noop = (): void => {
  // Nothing reads the chat again here.
};

const named = (readers: string[], others: number, notion = false) => ({
  title: "Client onboarding",
  audience: "named" as const,
  readers,
  others,
  notion,
});

function render(proposal: Proposal, more: Parameters<typeof mockApi>[0] = {}) {
  const keys: (string | null)[] = [];
  const states = new Map<string, string>();
  const mocked = mockApi({
    [APPEND]: (request) => {
      keys.push(request.headers.get("Idempotency-Key"));
      return {
        body: { append_id: "0192f1a0-0000-7000-8000-0000000007a1", blocks: 4, revision: 3 },
      };
    },
    [UNAPPEND]: (request) => {
      keys.push(request.headers.get("Idempotency-Key"));
      return { body: { removed: 4, stayed: 0 } };
    },
    [CREATE]: (request) => {
      keys.push(request.headers.get("Idempotency-Key"));
      return { body: { id: NEW_ID, title: "Kickoff notes" } };
    },
    [`POST ${W}/pages/${NEW_ID}/trash`]: (request) => {
      keys.push(request.headers.get("Idempotency-Key"));
      return { body: { id: NEW_ID } };
    },
    [SETTLE(proposal)]: (_request, body) => {
      states.set(proposal.id, (body as { state: string }).state);
      return { body: { ...proposal, state: (body as { state: Proposal["state"] }).state } };
    },
    [`GET ${W}/pages`]: { body: { pages: [CLIENT_ONBOARDING], tables: false, mail_scout: true } },
    [`GET ${W}/conversations`]: {
      body: { conversations: [MARTA, PEDRO], next_cursor: null },
    },
    ...more,
  });
  const root = createRootRoute({ component: Outlet });
  const card = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId",
    component: () => (
      <PageChangeCard
        workspaceId={DUARTE.id}
        chatId={MARTA_CHAT.id}
        proposal={proposal}
        onSettled={noop}
      />
    ),
  });
  const opened = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/pages/$pageId",
    component: () => <p>The page opened</p>,
  });
  const router = createRouter({
    routeTree: root.addChildren([card, opened]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}`] }),
  });
  return {
    mocked,
    keys,
    states,
    router,
    ...renderWithProviders(<RouterProvider router={router} />),
  };
}

describe("PageChangeCard", () => {
  it("shows where it goes, who reads it and the whole section; sends nothing until Add to page", async () => {
    const { mocked } = render(pageProposal());

    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    expect(within(card).getByText("Who will read it")).toBeVisible();
    expect(within(card).getByText("Only you")).toBeVisible();
    expect(
      within(card).getByRole("heading", { level: 2, name: "Kickoff notes — Acme Studio" }),
    ).toBeInTheDocument();
    expect(within(card).getAllByRole("listitem")).toHaveLength(3);
    // The threads it draws on, by their subject.
    expect(await within(card).findByRole("link", { name: /Walkthrough/u })).toBeVisible();
    expect(within(card).getByRole("button", { name: "Add to page" })).toBeEnabled();
    expect(within(card).getByRole("button", { name: "Edit first" })).toBeVisible();
    expect(within(card).getByRole("button", { name: "Discard" })).toBeVisible();
    expect(mocked.calls.some((call) => call.route === APPEND)).toBe(false);
  });

  it("names the teammates who read it, and counts the others", async () => {
    render(pageProposal({ page: named(["Rui Matos"], 2) }));
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    expect(within(card).getByText("You, Rui Matos and 2 others")).toBeVisible();
    expect(within(card).queryByText("Only you")).not.toBeInTheDocument();
    expect(within(card).queryByText("Sent to Notion too")).not.toBeInTheDocument();
  });

  it("says a count alone when no names are given", async () => {
    render(pageProposal({ page: named([], 3) }));
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    expect(within(card).getByText("You and 3 others")).toBeVisible();
  });

  it("says everyone in the workspace for a workspace page", async () => {
    render(pageProposal({ page: { ...named([], 0), audience: "everyone", others: 7 } }));
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    expect(within(card).getByText("Everyone in the workspace")).toBeVisible();
  });

  it("says when the page is kept in step with Notion", async () => {
    render(pageProposal({ page: named(["Rui Matos"], 0, true) }));
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    expect(within(card).getByText("Sent to Notion too")).toBeVisible();
  });

  it("folds a long section behind Show all, with all of it on the card", async () => {
    render(pageProposal({ markdown: LONG_NOTES }));
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    expect(within(card).getByText("Last checked on 28 Oct.")).toBeInTheDocument();
    const more = within(card).getByRole("button", { name: "Show all" });
    fireEvent.click(more);
    expect(within(card).getByRole("button", { name: "Show less" })).toBeVisible();
  });

  it.each([
    ["has no Markdown", () => pageProposal({ markdown: null })],
    ["does not say who reads the page", () => pageProposal({ page: null })],
    [
      "contradicts itself about who reads it",
      () =>
        pageProposal({ page: { ...named([], 0), audience: "only_you", readers: ["Rui Matos"] } }),
    ],
  ])("offers no Add to page when the proposal %s", async (_name, make) => {
    const { mocked } = render(make());
    const card = await screen.findByRole("region", { name: /./u });
    expect(
      await within(card).findByText(/doesn’t say what it would add or who would read it/u),
    ).toBeVisible();
    expect(within(card).queryByRole("button", { name: "Add to page" })).not.toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "Edit first" })).not.toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Discard" })).toBeVisible();
    expect(mocked.calls.some((call) => call.route === APPEND)).toBe(false);
  });

  it("adds the section once, keyed by the proposal, then offers Open page and Undo", async () => {
    const { mocked, keys, states } = render(pageProposal());
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });

    const add = within(card).getByRole("button", { name: "Add to page" });
    fireEvent.click(add);
    fireEvent.click(add);

    expect(await within(card).findByText("Added")).toBeVisible();
    const sent = mocked.calls.filter((call) => call.route === APPEND);
    expect(sent).toHaveLength(1);
    expect(sent[0]?.body).toMatchObject({ markdown: KICKOFF_NOTES });
    expect(keys).toEqual([pageProposal().id]);
    await waitFor(() => {
      expect(states.get(pageProposal().id)).toBe("done");
    });
    expect(within(card).getByRole("button", { name: "Open page" })).toBeVisible();
    expect(within(card).queryByRole("button", { name: "Add to page" })).not.toBeInTheDocument();
  });

  it("takes the section back with Undo (pages.unappend, by its append id)", async () => {
    const { mocked, keys, states } = render(pageProposal());
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    fireEvent.click(within(card).getByRole("button", { name: "Add to page" }));
    await within(card).findByText("Added");

    fireEvent.click(within(card).getByRole("button", { name: "Undo" }));

    expect(await within(card).findByText("Undone.")).toBeVisible();
    expect(mocked.calls.find((call) => call.route === UNAPPEND)?.body).toEqual({
      append_id: "0192f1a0-0000-7000-8000-0000000007a1",
    });
    expect(keys).toEqual([pageProposal().id, `${pageProposal().id}:undo`]);
    await waitFor(() => {
      expect(states.get(pageProposal().id)).toBe("undone");
    });
  });

  it("opens the page that was changed", async () => {
    const { router } = render(pageProposal());
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    fireEvent.click(within(card).getByRole("button", { name: "Add to page" }));
    fireEvent.click(await within(card).findByRole("button", { name: "Open page" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/pages/${PAGE}`);
    });
  });

  it("Edit first: the edited text is what is sent", async () => {
    const { mocked } = render(pageProposal());
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });

    fireEvent.click(within(card).getByRole("button", { name: "Edit first" }));
    const field = await within(card).findByRole("textbox", {
      name: "The text to add, as Markdown",
    });
    expect(field).toHaveValue(KICKOFF_NOTES);
    fireEvent.change(field, { target: { value: "## Kickoff notes\n\n- Brand files: sent" } });
    fireEvent.click(within(card).getByRole("button", { name: "Add to page" }));

    expect(await within(card).findByText("Added")).toBeVisible();
    expect(mocked.calls.find((call) => call.route === APPEND)?.body).toMatchObject({
      markdown: "## Kickoff notes\n\n- Brand files: sent",
    });
  });

  it("will not add text that is empty or over 16 KB", async () => {
    render(pageProposal());
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    fireEvent.click(within(card).getByRole("button", { name: "Edit first" }));
    const field = await within(card).findByRole("textbox", {
      name: "The text to add, as Markdown",
    });

    fireEvent.change(field, { target: { value: " ".repeat(3) } });
    expect(within(card).getByRole("button", { name: "Add to page" })).toBeDisabled();
    fireEvent.change(field, { target: { value: "é".repeat(9000) } });
    expect(within(card).getByRole("button", { name: "Add to page" })).toBeDisabled();
    expect(within(card).getByText(/over 16 KB/u)).toBeVisible();
  });

  it("makes a new page under its parent, opens it, and Undo puts it in the trash", async () => {
    const { mocked, router } = render(NEW_PAGE_PROPOSAL);
    const card = await screen.findByRole("region", { name: "New page under “Acme Studio”" });
    expect(within(card).getByText("Kickoff notes")).toBeVisible();
    expect(within(card).getByText("You and Rui Matos")).toBeVisible();

    fireEvent.click(within(card).getByRole("button", { name: "Add to page" }));
    expect(await within(card).findByText("Added")).toBeVisible();
    expect(mocked.calls.find((call) => call.route === CREATE)?.body).toMatchObject({
      title: "Kickoff notes",
      markdown: KICKOFF_NOTES,
    });

    fireEvent.click(within(card).getByRole("button", { name: "Open page" }));
    await waitFor(() => {
      expect(router.state.location.pathname).toBe(`/w/${DUARTE.id}/pages/${NEW_ID}`);
    });
  });

  it("puts the new page in the trash on Undo", async () => {
    const { mocked } = render(NEW_PAGE_PROPOSAL);
    const card = await screen.findByRole("region", { name: "New page under “Acme Studio”" });
    fireEvent.click(within(card).getByRole("button", { name: "Add to page" }));
    await within(card).findByText("Added");
    fireEvent.click(within(card).getByRole("button", { name: "Undo" }));
    expect(await within(card).findByText("Undone.")).toBeVisible();
    expect(mocked.calls.some((call) => call.route === `POST ${W}/pages/${NEW_ID}/trash`)).toBe(
      true,
    );
  });

  it("says why it could not be added, by the API's own reason", async () => {
    render(pageProposal(), {
      [APPEND]: {
        status: 409,
        body: { code: "page_trashed", title: "Trashed", detail: "That page is in the trash." },
      },
    });
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    fireEvent.click(within(card).getByRole("button", { name: "Add to page" }));
    expect(
      await within(card).findByText("This couldn’t be done: That page is in the trash."),
    ).toBeVisible();
  });

  it("Discard settles it as declined and changes nothing", async () => {
    const { mocked, states } = render(pageProposal());
    const card = await screen.findByRole("region", { name: "Add to “Client onboarding”" });
    fireEvent.click(within(card).getByRole("button", { name: "Discard" }));
    expect(await within(card).findByText("Not now.")).toBeVisible();
    await waitFor(() => {
      expect(states.get(pageProposal().id)).toBe("declined");
    });
    expect(mocked.calls.some((call) => call.route === APPEND)).toBe(false);
  });

  it("never sends a path that is not the page's own", async () => {
    const odd = pageProposal();
    const { mocked } = render({
      ...odd,
      request: {
        operation: "pages.append",
        method: "POST",
        path: `../../workspaces/${DUARTE.id}`,
        body: null,
      },
    });
    const card = await screen.findByRole("region", { name: /./u });
    expect(within(card).queryByRole("button", { name: "Add to page" })).not.toBeInTheDocument();
    expect(mocked.calls.every((call) => !call.route.includes("workspaces"))).toBe(true);
  });
});
