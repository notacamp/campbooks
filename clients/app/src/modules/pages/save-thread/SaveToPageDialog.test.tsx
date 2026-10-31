import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import * as Y from "yjs";

import { ScoutChatProvider, type OpenScoutChat } from "~/lib/scout-chat";

import { W } from "../../../../test/demo/conversations";
import { CLIENT_ONBOARDING, HANDBOOK } from "../../../../test/demo/pages";
import {
  forRequests,
  HEAVY_THREAD,
  KICKOFF_ID,
  KICKOFF_THREAD,
  saveToPageServer,
  type SaveServerOptions,
} from "../../../../test/demo/save-to-page";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";

import { SaveToPageDialog } from "./SaveToPageDialog";
import type { SaveThread } from "./thread";

function setup(
  options: SaveServerOptions & { scout?: OpenScoutChat | undefined; thread?: SaveThread } = {},
) {
  const { scout, thread = KICKOFF_THREAD, ...server } = options;
  const pages = saveToPageServer(server);
  const mocked = mockApi(forRequests(pages.routes));
  const onClose = vi.fn();
  const root = createRootRoute({
    component: () => (
      <SaveToPageDialog workspaceId={DUARTE.id} thread={thread} open onClose={onClose} />
    ),
  });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  const tree = <RouterProvider router={router} />;
  const rendered = renderWithProviders(
    scout === undefined ? tree : <ScoutChatProvider open={scout}>{tree}</ScoutChatProvider>,
  );
  return { pages, mocked, onClose, ...rendered };
}

const docOf = (pages: { docs: Map<string, Y.Doc> }, id: string): Y.Doc => {
  const doc = pages.docs.get(id);
  if (doc === undefined) {
    throw new Error("the page was never read");
  }
  return doc;
};

const types = (doc: Y.Doc) =>
  doc
    .getXmlFragment("default")
    .slice(0)
    .map((node) => (node instanceof Y.XmlElement ? node.nodeName : "text"));

/** Wait for the dialog and the pages it lists. */
async function opened() {
  const dialog = await screen.findByRole("dialog", { name: "Save “Kickoff agenda” to a page" });
  await within(dialog).findByRole("option", { name: /Client onboarding/u });
  return dialog;
}

const pick = (dialog: HTMLElement, name: RegExp | string) => {
  fireEvent.click(within(dialog).getByRole("option", { name }));
};

describe("Save to page", () => {
  it("keeps Cancel and Save in a footer of their own, outside the scrolling body", async () => {
    // jsdom has no layout, so the phone sheet's visible box cannot be measured here;
    // the structure that pins the footer (head, scrolling body, footer) can.
    setup();
    const dialog = await opened();
    const save = within(dialog).getByRole("button", { name: /^Save/u });
    const search = within(dialog).getByRole("combobox");
    const topmost = (from: HTMLElement, other: HTMLElement) => {
      let at = from;
      while (at.parentElement !== null && !at.parentElement.contains(other)) {
        at = at.parentElement;
      }
      return at;
    };
    const body = topmost(search, save);
    const footer = topmost(save, search);
    // Siblings under the panel, the footer after the body: it is not part of what scrolls.
    expect(body.parentElement).toBe(footer.parentElement);
    expect(body.compareDocumentPosition(footer) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("lists the pages already linking the thread first, by name, leaving out what Ana may only read", async () => {
    setup({ linked: [HANDBOOK.id] });
    const dialog = await opened();
    const options = within(dialog).getAllByRole("option");
    expect(options[0]).toHaveTextContent("Team handbook");
    expect(options[0]).toHaveTextContent("Already links this thread");
    // Rui's draft is Ana's to read only.
    expect(within(dialog).queryByRole("option", { name: /Budget draft/u })).not.toBeInTheDocument();
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Find a page by name" }), {
      target: { value: "acme" },
    });
    expect(within(dialog).getAllByRole("option")).toHaveLength(2);
    expect(within(dialog).getByRole("option", { name: /^Acme Studio/u })).toBeVisible();
  });

  it("saves a live thread block to a page, then Open page and Undo", async () => {
    const { pages, onClose } = setup();
    const dialog = await opened();
    pick(dialog, /Client onboarding/u);
    fireEvent.click(within(dialog).getByRole("button", { name: /^Save/u }));

    await waitFor(() => {
      expect(onClose).toHaveBeenCalled();
    });
    expect(types(docOf(pages, CLIENT_ONBOARDING.id))).toEqual(["paragraph", "embed"]);
    const embed = pages.docs.get(CLIENT_ONBOARDING.id)?.getXmlFragment("default").get(1);
    expect((embed as Y.XmlElement).getAttribute("ref_id")).toBe(KICKOFF_ID);
    expect((await screen.findAllByText("Saved to Client onboarding")).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Open page" })).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(types(docOf(pages, CLIENT_ONBOARDING.id))).toEqual(["paragraph"]);
    });
  });

  it("saves with ⌘↵", async () => {
    const { pages } = setup();
    const dialog = await opened();
    pick(dialog, /Client onboarding/u);
    fireEvent.keyDown(within(dialog).getByRole("combobox"), { key: "Enter", ctrlKey: true });
    await waitFor(() => {
      expect(types(docOf(pages, CLIENT_ONBOARDING.id))).toEqual(["paragraph", "embed"]);
    });
  });

  it("saves a copy: a heading per message, then its words", async () => {
    const { pages } = setup();
    const dialog = await opened();
    pick(dialog, /Client onboarding/u);
    fireEvent.click(within(dialog).getByRole("radio", { name: /A copy of the messages/u }));
    fireEvent.click(within(dialog).getByRole("button", { name: /^Save/u }));
    await waitFor(() => {
      expect(types(docOf(pages, CLIENT_ONBOARDING.id))).toEqual([
        "paragraph",
        "heading",
        "paragraph",
        "heading",
        "paragraph",
      ]);
    });
  });

  describe("a copy says who will read the page before Ana saves", () => {
    it("names Rui for a page Rui reads", async () => {
      setup();
      const dialog = await opened();
      pick(dialog, /^Acme Studio/u);
      fireEvent.click(within(dialog).getByRole("radio", { name: /A copy of the messages/u }));
      expect(await within(dialog).findByText("Rui Matos will read it")).toBeVisible();
    });

    it("says everyone for a workspace page, and Notion when it is kept in step", async () => {
      setup({ linked: [HANDBOOK.id] });
      const dialog = await opened();
      pick(dialog, /Team handbook/u);
      fireEvent.click(within(dialog).getByRole("radio", { name: /A copy of the messages/u }));
      expect(
        await within(dialog).findByText("Everyone in the workspace will read it"),
      ).toBeVisible();
      expect(within(dialog).getByText("It’s sent to Notion too")).toBeVisible();
    });

    it("says only you for a new page", async () => {
      setup();
      const dialog = await opened();
      fireEvent.click(within(dialog).getByRole("button", { name: "New page" }));
      fireEvent.click(within(dialog).getByRole("radio", { name: /A copy of the messages/u }));
      expect(within(dialog).getByText("Only you will read it")).toBeVisible();
    });
  });

  it("makes a new private page at the top level and puts it back in the trash on Undo", async () => {
    const { pages, mocked } = setup();
    const dialog = await opened();
    fireEvent.click(within(dialog).getByRole("button", { name: "New page" }));
    const title = within(dialog).getByRole("textbox", { name: "Title" });
    expect(title).toHaveValue("Kickoff agenda");
    fireEvent.change(title, { target: { value: "Kickoff notes" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /^Save/u }));

    await waitFor(() => {
      expect(mocked.calls.find((call) => call.route === `POST ${W}/pages`)?.body).toMatchObject({
        title: "Kickoff notes",
        place: "private",
        parent: null,
      });
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(pages.trashed.size).toBe(1);
    });
  });

  describe("attachments", () => {
    it("attaches the files and adds their blocks in the same save, and Undo lets go of them", async () => {
      const { pages } = setup();
      const dialog = await opened();
      pick(dialog, /Client onboarding/u);
      fireEvent.click(within(dialog).getByRole("checkbox", { name: "Include attachments (2)" }));
      fireEvent.click(within(dialog).getByRole("button", { name: /^Save/u }));
      await waitFor(() => {
        expect(types(docOf(pages, CLIENT_ONBOARDING.id))).toEqual([
          "paragraph",
          "embed",
          "file",
          "file",
        ]);
      });
      expect(pages.held.size).toBe(2);
      fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
      await waitFor(() => {
        expect(pages.held.size).toBe(0);
      });
      expect(types(docOf(pages, CLIENT_ONBOARDING.id))).toEqual(["paragraph"]);
    });

    it("says which file is over 20 MB and adds nothing", async () => {
      const { pages } = setup({ thread: HEAVY_THREAD });
      const dialog = await opened();
      pick(dialog, /Client onboarding/u);
      fireEvent.click(within(dialog).getByRole("checkbox", { name: "Include attachments (1)" }));
      expect(
        await within(dialog).findByText(
          "“footage.mov” is over 20 MB, so it can’t be saved to a page. Nothing was added.",
        ),
      ).toBeVisible();
      expect(within(dialog).getByRole("button", { name: /^Save/u })).toBeDisabled();
      expect(pages.calls.some((call) => call.includes("from-message"))).toBe(false);
      expect(types(pages.docs.get(CLIENT_ONBOARDING.id) ?? new Y.Doc())).toEqual([]);
    });

    it.each([
      [403, "insufficient_scope", /may not read mail/u],
      [404, undefined, /thread changed, or a file is gone/u],
      [413, "file_too_large", /over 20 MB/u],
    ])("says a %s refusal by name, and adds nothing", async (status, code, words) => {
      const { pages, onClose } = setup({ attachRefusal: { status, ...(code ? { code } : {}) } });
      const dialog = await opened();
      pick(dialog, /Client onboarding/u);
      fireEvent.click(within(dialog).getByRole("checkbox", { name: "Include attachments (2)" }));
      fireEvent.click(within(dialog).getByRole("button", { name: /^Save/u }));
      expect(await within(dialog).findByText(words)).toBeVisible();
      expect(onClose).not.toHaveBeenCalled();
      // The page is as it was.
      expect(types(pages.docs.get(CLIENT_ONBOARDING.id) ?? new Y.Doc())).toEqual([]);
      expect(pages.calls.some((call) => call.endsWith("/sync"))).toBe(false);
    });
  });

  describe("Scout's summary", () => {
    it("is hidden when Scout is not there", async () => {
      setup();
      const dialog = await opened();
      expect(
        within(dialog).getByRole("radio", { name: /A live thread block/u }),
      ).toBeInTheDocument();
      expect(
        within(dialog).getByRole("radio", { name: /A copy of the messages/u }),
      ).toBeInTheDocument();
      expect(
        within(dialog).queryByRole("radio", { name: /Scout’s summary/u }),
      ).not.toBeInTheDocument();
    });

    it("turns Save into Preview summary and hands over to Scout's chat about the conversation", async () => {
      const scout = vi.fn();
      const { pages, onClose } = setup({ scout });
      const dialog = await opened();
      pick(dialog, /Client onboarding/u);
      fireEvent.click(within(dialog).getByRole("radio", { name: /Scout’s summary/u }));
      fireEvent.click(within(dialog).getByRole("button", { name: /^Preview summary/u }));

      expect(onClose).toHaveBeenCalled();
      expect(scout).toHaveBeenCalledWith(
        { type: "conversations.conversation", id: KICKOFF_ID },
        expect.objectContaining({
          ask: expect.stringContaining("“Client onboarding”") as string,
        }),
      );
      // Nothing is added until the member chooses Add to page on Scout's card.
      expect(pages.calls.some((call) => call.endsWith("/sync"))).toBe(false);
    });
  });

  it("says why it could not save, and keeps the dialog open", async () => {
    const { onClose } = setup({});
    const dialog = await opened();
    pick(dialog, /^Acme Studio/u);
    // The page went away between the list and the save.
    globalThis.fetch = () => Promise.reject(new TypeError("offline"));
    fireEvent.click(within(dialog).getByRole("button", { name: /^Save/u }));
    expect(
      await within(dialog).findByText("Couldn’t save to the page. Nothing was added."),
    ).toBeVisible();
    expect(onClose).not.toHaveBeenCalled();
  });
});
