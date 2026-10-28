import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { createContext, use, useState, type ReactNode } from "react";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { ScoutChatProvider } from "~/lib/scout-chat";
import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { PageChangeCard } from "../../src/modules/assistant/components/PageChange/PageChangeCard";
import type { Proposal } from "../../src/modules/assistant/api/proposals";
import { SaveToPageDialog, type SaveThread } from "../../src/modules/pages";
import {
  KICKOFF_NOTES,
  LONG_NOTES,
  MARTA_CHAT,
  NEW_PAGE_PROPOSAL,
  pageProposal,
} from "../../test/demo/assistant";
import { COUNTS, MARTA } from "../../test/demo/attention";
import { INBOX, STREAM_COUNTS, W } from "../../test/demo/conversations";
import { CLIENT_ONBOARDING } from "../../test/demo/pages";
import {
  HEAVY_THREAD,
  KICKOFF_THREAD,
  saveToPageServer,
  type Handler,
  type SaveServerOptions,
} from "../../test/demo/save-to-page";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Mail and Scout in Pages (campbooks-cloud#364), against fictional demo data
// (Ana saves Jonas Weber's "Kickoff agenda" thread to "Client onboarding"):
// Save to page from the reader, Scout's page change card, and Ask Scout on a
// page. The pretend server keeps state, and starts over each time a story
// loads. The `*Idle` stories have no `play`, for the recording.

/** The pretend server's routes in the stories' mock. */
const forStories = (routes: Record<string, Handler>): Record<string, MockRoute> =>
  Object.fromEntries(
    Object.entries(routes).map(([route, handler]) => [route, (body, url) => handler(body, url)]),
  );

/** What the story's router draws: the part under test. */
const Content = createContext<ReactNode>(null);

function Slot() {
  return use(Content);
}

/** The providers a screen part needs: queries, the undo toast, a router, and maybe Scout. */
function Host({ children, scout }: { children: ReactNode; scout?: (() => void) | undefined }) {
  const [app] = useState(() => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const root = createRootRoute({ component: Slot });
    const router = createRouter({
      routeTree: root,
      history: createMemoryHistory({ initialEntries: ["/"] }),
    });
    return { queryClient, router };
  });
  const tree = (
    <Content value={children}>
      <RouterProvider router={app.router} />
    </Content>
  );
  return (
    <QueryClientProvider client={app.queryClient}>
      <UndoProvider>
        {scout === undefined ? tree : <ScoutChatProvider open={scout}>{tree}</ScoutChatProvider>}
      </UndoProvider>
    </QueryClientProvider>
  );
}

/** The dialog, open, which closes when it is done (as in the reader). */
function OpenDialog({ thread }: { thread: SaveThread }) {
  const [open, setOpen] = useState(true);
  return (
    <SaveToPageDialog
      workspaceId={DUARTE.id}
      thread={thread}
      open={open}
      onClose={() => {
        setOpen(false);
      }}
    />
  );
}

interface DialogArgs {
  server: SaveServerOptions;
  thread: SaveThread;
  scout: boolean;
}

interface CardArgs {
  proposal: Proposal;
}

interface AskArgs {
  server: SaveServerOptions;
}

const openScout = fn();

const meta = {
  title: "Screens/Pages from mail and Scout",
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;

// --- Save to page --------------------------------------------------------

const dialog = {
  beforeEach: ({ args, globals }: { args: DialogArgs; globals: Record<string, unknown> }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(forStories(saveToPageServer(args.server).routes));
  },
  render: ({ thread, scout }: DialogArgs) => (
    <Host scout={scout ? openScout : undefined}>
      <OpenDialog thread={thread} />
    </Host>
  ),
};

type DialogStory = StoryObj<DialogArgs>;
const base: DialogArgs = { server: {}, thread: KICKOFF_THREAD, scout: true };

/** A viewport the story test runs at (the addon reads `globals.viewport`). */
function at(name: string, width: number, height: number) {
  return {
    parameters: {
      viewport: {
        options: {
          [name]: { name, styles: { width: `${String(width)}px`, height: `${String(height)}px` } },
        },
      },
    },
    globals: { viewport: { value: name } },
  };
}

const body = (canvasElement: HTMLElement) => within(canvasElement.ownerDocument.body);

/** The page picker finds a page by name; the pages already linking the thread come first. */
export const PickAPage: DialogStory = {
  ...dialog,
  args: base,
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    const options = await screen.findAllByRole("option");
    await expect(options[0]).toHaveTextContent("Client onboarding");
    await expect(options[0]).toHaveTextContent("Already links this thread");
    await userEvent.type(screen.getByRole("combobox", { name: "Find a page by name" }), "acme");
    await expect(screen.getAllByRole("option")).toHaveLength(2);
  },
};

/** A live thread block goes to the end of the page, with Open page and Undo. */
export const SaveALiveBlock: DialogStory = {
  ...dialog,
  args: base,
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("option", { name: /Client onboarding/u }));
    await userEvent.click(screen.getByRole("button", { name: /^Save/u }));
    await expect((await screen.findAllByText("Saved to Client onboarding"))[0]).toBeVisible();
    await expect(screen.getByRole("button", { name: "Open page" })).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: /^Undo/u }));
  },
};

/** A copy of the messages says who will read the page before it is saved. */
export const CopyForASharedPage: DialogStory = {
  ...dialog,
  args: base,
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("option", { name: /^Acme Studio/u }));
    await userEvent.click(screen.getByRole("radio", { name: /A copy of the messages/u }));
    await expect(await screen.findByText("Rui Matos will read it")).toBeVisible();
  },
};

/** A new page is private, at the top level. */
export const NewPage: DialogStory = {
  ...dialog,
  args: base,
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("button", { name: "New page" }));
    await expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("Kickoff agenda");
    await expect(screen.getByText(/A private page at the top level/u)).toBeInTheDocument();
  },
};

/** The attachments come along as files of the page. */
export const WithAttachments: DialogStory = {
  ...dialog,
  args: base,
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("option", { name: /Client onboarding/u }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Include attachments (2)" }));
    await userEvent.click(screen.getByRole("button", { name: /^Save/u }));
    await expect((await screen.findAllByText("Saved to Client onboarding"))[0]).toBeVisible();
  },
};

/** A file over 20 MB is named, and nothing is added. */
export const FileTooLarge: DialogStory = {
  ...dialog,
  args: { ...base, thread: HEAVY_THREAD },
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("option", { name: /Client onboarding/u }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Include attachments (1)" }));
    await expect(await screen.findByText(/“footage.mov” is over 20 MB/u)).toBeVisible();
    await expect(screen.getByRole("button", { name: /^Save/u })).toBeDisabled();
  },
};

/** The server says this sign-in may not read mail: said by name, nothing added. */
export const RefusedForScope: DialogStory = {
  ...dialog,
  args: { ...base, server: { attachRefusal: { status: 403, code: "insufficient_scope" } } },
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("option", { name: /Client onboarding/u }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Include attachments (2)" }));
    await userEvent.click(screen.getByRole("button", { name: /^Save/u }));
    await expect(await screen.findByText(/may not read mail/u)).toBeVisible();
  },
};

/** Scout's summary turns Save into Preview summary and hands over to Scout's chat. */
export const ScoutSummary: DialogStory = {
  ...dialog,
  args: base,
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await userEvent.click(await screen.findByRole("option", { name: /Client onboarding/u }));
    await userEvent.click(screen.getByRole("radio", { name: /Scout’s summary/u }));
    await userEvent.click(screen.getByRole("button", { name: /^Preview summary/u }));
    await waitFor(() => expect(openScout).toHaveBeenCalled());
  },
};

/** Without Scout the summary is not offered. */
export const WithoutScout: DialogStory = {
  ...dialog,
  args: { ...base, scout: false },
  play: async ({ canvasElement }) => {
    const screen = body(canvasElement);
    await screen.findByRole("radio", { name: /A live thread block/u });
    await expect(screen.queryByRole("radio", { name: /Scout’s summary/u })).toBeNull();
  },
};

/** The dialog, nothing done yet (the recording picks, saves and undoes). */
export const DialogIdle: DialogStory = { ...dialog, args: base, ...at("desktop", 1440, 900) };

/** The dialog on a phone: a sheet. */
export const DialogPhoneIdle: DialogStory = { ...dialog, args: base, ...at("phone", 375, 812) };

// --- Scout's page change card --------------------------------------------

const card = {
  beforeEach: ({ args, globals }: { args: CardArgs; globals: Record<string, unknown> }) => {
    saveThemeFromGlobals(globals);
    const done = (state: string) => ({ body: { ...args.proposal, state } });
    const proposalPath = `${W}/assistant/chats/${MARTA_CHAT.id}/proposals/${args.proposal.id}`;
    return mockFetch({
      [`GET ${W}/pages`]: { body: { pages: [CLIENT_ONBOARDING], tables: false } },
      [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
      [`POST ${W}/pages/${CLIENT_ONBOARDING.id}/append`]: {
        body: { append_id: "0192f1a0-0000-7000-8000-0000000007a1", blocks: 4, revision: 3 },
      },
      [`POST ${W}/pages/${CLIENT_ONBOARDING.id}/unappend`]: { body: { removed: 4, stayed: 0 } },
      [`POST ${W}/pages`]: { status: 201, body: { id: "0192f1a0-0000-7000-8000-0000000009ff" } },
      [`POST ${W}/pages/0192f1a0-0000-7000-8000-0000000009ff/trash`]: { body: {} },
      [`POST ${proposalPath}`]: (request) => done((request as { state: string }).state),
    });
  },
  render: ({ proposal }: CardArgs) => (
    <Host>
      <div className="max-w-prose mx-auto flex w-full flex-col gap-3 p-4">
        <PageChangeCard
          workspaceId={DUARTE.id}
          chatId={MARTA_CHAT.id}
          proposal={proposal}
          onSettled={fn()}
        />
      </div>
    </Host>
  ),
};

type CardStory = StoryObj<CardArgs>;
const named = (readers: string[], others: number, notion = false) => ({
  title: "Client onboarding",
  audience: "named" as const,
  readers,
  others,
  notion,
});
const region = (canvasElement: HTMLElement, name: string | RegExp) =>
  within(canvasElement).findByRole("region", { name });

/** Only Ana reads the page. */
export const CardOnlyYou: CardStory = {
  ...card,
  args: { proposal: pageProposal() },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await expect(section.getByText("Only you")).toBeVisible();
    await expect(section.getByRole("button", { name: "Add to page" })).toBeEnabled();
  },
};

/** Named teammates and a count of the others. */
export const CardNamedReaders: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: named(["Rui Matos"], 2) }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await expect(section.getByText("You, Rui Matos and 2 others")).toBeVisible();
  },
};

/** A count alone when no names are given. */
export const CardCountAlone: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: named([], 3) }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await expect(section.getByText("You and 3 others")).toBeVisible();
  },
};

/** A workspace page: everyone reads it. */
export const CardEveryone: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: { ...named([], 0), audience: "everyone", others: 7 } }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await expect(section.getByText("Everyone in the workspace")).toBeVisible();
  },
};

/** A page kept in step with Notion says so. */
export const CardNotion: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: named(["Rui Matos"], 0, true) }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await expect(section.getByText("Sent to Notion too")).toBeVisible();
  },
};

/** A long section folds behind Show all. */
export const CardLongMarkdown: CardStory = {
  ...card,
  args: { proposal: pageProposal({ markdown: LONG_NOTES }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await userEvent.click(section.getByRole("button", { name: "Show all" }));
    await expect(section.getByRole("button", { name: "Show less" })).toBeVisible();
  },
};

/** A proposal that does not say who reads the page has no Add to page. */
export const CardMissingReaders: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: null }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, /./u));
    await expect(
      section.getByText(/doesn’t say what it would add or who would read it/u),
    ).toBeVisible();
    await expect(section.queryByRole("button", { name: "Add to page" })).toBeNull();
    await expect(section.getByRole("button", { name: "Discard" })).toBeVisible();
  },
};

/** Edit first: the edited text is what is added. */
export const CardEditFirst: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: named(["Rui Matos"], 0) }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await userEvent.click(section.getByRole("button", { name: "Edit first" }));
    const field = await section.findByRole("textbox", { name: "The text to add, as Markdown" });
    await expect(field).toHaveValue(KICKOFF_NOTES);
  },
};

/** Added: Open page and Undo. */
export const CardAdded: CardStory = {
  ...card,
  args: { proposal: pageProposal({ page: named(["Rui Matos"], 0, true) }) },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "Add to “Client onboarding”"));
    await userEvent.click(section.getByRole("button", { name: "Add to page" }));
    await expect(await section.findByText("Added")).toBeVisible();
    await expect(section.getByRole("button", { name: "Open page" })).toBeVisible();
    await userEvent.click(section.getByRole("button", { name: "Undo" }));
    await expect(await section.findByText("Undone.")).toBeVisible();
  },
};

/** A new page under another. */
export const CardNewPage: CardStory = {
  ...card,
  args: { proposal: NEW_PAGE_PROPOSAL },
  play: async ({ canvasElement }) => {
    const section = within(await region(canvasElement, "New page under “Acme Studio”"));
    await expect(section.getByText("You and Rui Matos")).toBeVisible();
  },
};

/** Readers, Notion and a long section: the recording folds, adds and undoes. */
export const CardIdle: CardStory = {
  ...card,
  args: {
    proposal: pageProposal({
      markdown: LONG_NOTES,
      page: named(["Rui Matos"], 2, true),
    }),
  },
  ...at("desktop", 1440, 900),
};

/** The same card on a phone: the buttons wrap. */
export const CardPhoneIdle: CardStory = {
  ...card,
  args: {
    proposal: pageProposal({
      markdown: LONG_NOTES,
      page: named(["Rui Matos"], 2, true),
    }),
  },
  ...at("phone", 375, 812),
};

// --- Ask Scout on a page -------------------------------------------------

const PAGE_CHAT = {
  ...MARTA_CHAT,
  id: "0192f1a0-0000-7000-8000-000000003801",
  about: { type: "pages.page", id: CLIENT_ONBOARDING.id },
};

function AppAt({ path }: { path: string }) {
  const [app] = useState(() => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createAppRouter(queryClient, createMemoryHistory({ initialEntries: [path] }));
    return { queryClient, router };
  });
  return (
    <QueryClientProvider client={app.queryClient}>
      <UndoProvider>
        <RouterProvider router={app.router} />
      </UndoProvider>
    </QueryClientProvider>
  );
}

const ask = {
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }: { args: AskArgs; globals: Record<string, unknown> }) => {
    saveThemeFromGlobals(globals);
    return mockFetch({
      ...forStories(saveToPageServer(args.server).routes),
      "GET /bff/session": { body: SESSION },
      "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
      [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
      [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
      [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
      [`GET ${W}/tasks`]: { body: { tasks: [] } },
      [`GET ${W}/correspondents`]: { body: { correspondents: [] } },
      [`GET ${W}/attention`]: { body: { items: [], counts: COUNTS } },
      [`POST ${W}/assistant/chats`]: { status: 201, body: PAGE_CHAT },
    });
  },
  render: () => <AppAt path={`/w/${DUARTE.id}/pages/${CLIENT_ONBOARDING.id}`} />,
};

type AskStory = StoryObj<AskArgs>;

/** The page's More menu offers Ask Scout; the toolbar stays one row. */
export const AskScoutOnAPage: AskStory = {
  ...ask,
  args: { server: {} },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "More" }));
    await userEvent.click(await body(canvasElement).findByRole("button", { name: /^Ask Scout/u }));
    await expect(await body(canvasElement).findByText(/Page: Client onboarding/u)).toBeVisible();
  },
};

/** While the server does not offer mail and Scout in pages the menu has no Ask Scout. */
export const AskScoutOffWhileTheServerDoesNotOfferIt: AskStory = {
  ...ask,
  args: { server: { off: true } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "More" }));
    await expect(
      await body(canvasElement).findByRole("button", { name: "Move to trash" }),
    ).toBeVisible();
    await expect(body(canvasElement).queryByRole("button", { name: /^Ask Scout/u })).toBeNull();
  },
};

/** The page, nothing done yet (the recording opens the menu and asks). */
export const AskScoutIdle: AskStory = { ...ask, args: { server: {} }, ...at("desktop", 1440, 900) };
