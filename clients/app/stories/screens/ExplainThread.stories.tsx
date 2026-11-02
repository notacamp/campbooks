import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { W } from "../../test/demo/conversations";
import {
  ANAS_INBOX,
  KICKOFF,
  KICKOFF_STALE,
  kickoffDetail,
  kickoffExplained,
  type Explained,
} from "../../test/demo/kickoff";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, type MockRoute } from "./mock-fetch";

// Scout explains a thread (campbooks-cloud#407), in the real inbox against
// demo data: nothing until you ask, Scout reading, the explanation with the
// messages each point comes from, one line when you come back, and what the
// card says when the thread moved on, the day's AI is used up, or Scout
// could not. The in-memory server writes an explanation 2.5 s after the ask.

const PATH = `/w/${DUARTE.id}/inbox/primary/${KICKOFF.id}`;
const EXPLANATION = `${W}/conversations/${KICKOFF.id}/explanation`;
/** How long the in-memory Scout takes to write. */
const WRITES_IN = 2500;

interface Memory {
  /** What the member's row holds now; none until they ask. */
  row: Explained | undefined;
  /** When the last ask was made. */
  askedAt: number;
}

/** The mailbox with the thread, and a Scout that writes `after` an ask. */
function server(options: {
  row?: Explained | undefined;
  newer?: boolean;
  explain?: boolean;
  /** What an ask turns into once written; `writing` never finishes. */
  result?: Explained | "writing" | "failed";
  after?: number;
}): Record<string, MockRoute> {
  const memory: Memory = { row: options.row, askedAt: 0 };
  const result = options.result ?? kickoffExplained({ messages: options.newer === true ? 10 : 9 });
  const settle = (): Explained | undefined => {
    const row = memory.row;
    if (row?.state !== "writing" || result === "writing") {
      return row;
    }
    if (Date.now() - memory.askedAt >= (options.after ?? WRITES_IN)) {
      memory.row =
        result === "failed"
          ? { ...row, state: "failed", failure: "unusable", body: null, refs: [] }
          : result;
    }
    return memory.row;
  };
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [ANAS_INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: [] } },
    [`GET ${W}/conversations`]: {
      body: {
        conversations: [options.newer === true ? KICKOFF_STALE : KICKOFF],
        next_cursor: null,
      },
    },
    [`GET ${W}/conversations/${KICKOFF.id}`]: {
      body: kickoffDetail({ newer: options.newer === true, explain: options.explain !== false }),
    },
    [`GET ${EXPLANATION}`]: () => {
      const row = settle();
      return row === undefined
        ? { status: 404, body: { status: 404, code: "not_found", title: "Not found" } }
        : { body: row };
    },
    [`POST ${EXPLANATION}`]: () => {
      memory.askedAt = Date.now();
      memory.row = {
        state: "writing",
        locale: memory.row?.locale ?? null,
        written_at: memory.row?.written_at ?? null,
        messages: memory.row?.messages ?? 0,
        in_full: memory.row?.in_full ?? 0,
        stale: false,
        body: memory.row?.body ?? null,
        refs: memory.row?.refs ?? [],
      };
      return { status: 202, body: memory.row };
    },
  };
}

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

interface ScreenArgs {
  path: string;
  api: Record<string, MockRoute>;
}

const meta = {
  title: "Screens/Explain a thread",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args }) => mockFetch(args.api),
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

const CARD = "Scout’s explanation";

/** Click the note's "Explain this thread" (the More menu repeats it). */
async function ask(canvasElement: HTMLElement) {
  const canvas = within(canvasElement);
  const [button] = await canvas.findAllByRole("button", { name: /Explain this thread/u });
  if (button !== undefined) {
    await userEvent.click(button);
  }
}

/** A thread nobody asked about: nothing but a quiet button in the note. */
export const Offered: Story = {
  args: { path: PATH, api: server({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      (await canvas.findAllByRole("button", { name: /Explain this thread/u }))[0],
    ).toBeVisible();
    await expect(canvas.queryByRole("region", { name: CARD })).toBeNull();
  },
};

/** Scout reads for as long as you wait: the skeleton, and Stop only hides it. */
export const Writing: Story = {
  args: { path: PATH, api: server({ result: "writing" }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await ask(canvasElement);
    await expect(await canvas.findByText("Scout is reading the 9 messages…")).toBeVisible();
  },
};

/** Right after you ask: the explanation arrives whole. */
export const Asked: Story = {
  args: { path: PATH, api: server({ after: 300 }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await ask(canvasElement);
    // Written within a poll or two (1.5 s each).
    await expect(
      await canvas.findByText(/Acme Studio wants the kickoff/u, undefined, { timeout: 6000 }),
    ).toBeVisible();
  },
};

/** Coming back to a thread you asked about: one line, and nothing opens by itself. */
export const ComingBack: Story = {
  args: { path: PATH, api: server({ row: kickoffExplained() }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText(/Your explanation from .* · from 9 messages/u),
    ).toBeVisible();
    await expect(canvas.queryByRole("region", { name: CARD })).toBeNull();
  },
};

/** A newer message arrived: the line says who wrote, and offers Explain again. */
export const AfterANewMessage: Story = {
  args: {
    path: PATH,
    api: server({
      newer: true,
      row: kickoffExplained({
        stale: true,
        newer: { from: "Rui Matos", sent_at: "2026-10-26T11:05:00Z" },
      }),
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText(/Rui wrote since/u)).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Explain again" })).toBeVisible();
  },
};

/** The workspace used the day's AI: any earlier explanation stays, with what happened. */
export const LimitReached: Story = {
  args: {
    path: PATH,
    api: server({ row: kickoffExplained({ state: "failed", failure: "limit", stale: true }) }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Show" }));
    await expect(await canvas.findByText(/used today’s AI/u)).toBeVisible();
  },
};

/** Scout could not explain it: it says so and offers to try again. */
export const Unusable: Story = {
  args: {
    path: PATH,
    api: server({
      row: kickoffExplained({ state: "failed", failure: "unusable", body: null, refs: [] }),
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Show" }));
    await expect(await canvas.findByRole("button", { name: "Try again" })).toBeVisible();
  },
};

/** A long thread says what it rests on. */
export const LongThread: Story = {
  args: { path: PATH, api: server({ row: kickoffExplained({ messages: 34, in_full: 20 }) }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Show" }));
    await expect(await canvas.findByText(/the newest 20 in full/u)).toBeVisible();
  },
};

/** The server does not offer it (no model that may read mail, or a phishing warning): no button. */
export const NotOffered: Story = {
  args: { path: PATH, api: server({ explain: false }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { name: "Kickoff agenda" });
    await expect(canvas.queryByRole("button", { name: /Explain this thread/u })).toBeNull();
  },
};
