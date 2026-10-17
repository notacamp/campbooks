import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { INBOX, W } from "../../test/demo/conversations";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Scout's questions, in the real app against in-memory demo data: the first
// view's filter box finding a sender by name, tested with axe in light and
// dark. Fictional people only.

const PAGE = `/scout/${DUARTE.id}`;
const PROFILE = { proactivity: "ask_first", asked_at: null, revision: 0 };

/** The ten replied to most, then people only a search finds. */
const SUGGESTED = [
  ["Maya Okafor", "maya@okafor-studio.example", 41, 23],
  ["Rita Gomes", "rita@silva-contas.example", 38, 19],
  ["Duarte Pessoa", "duarte@pessoa-obras.example", 28, 17],
  ["Leo Martins", "leo@martins-lda.example", 24, 12],
  ["Ana Lopes", "ana@lopes-arquitetura.example", 19, 11],
  ["Tomás Reis", "tomas@reis-transportes.example", 17, 9],
  ["Clara Neves", "clara@neves-design.example", 15, 8],
  ["Bruno Dias", "bruno@dias-contabilidade.example", 12, 6],
  ["Inês Sousa", "ines@ateliersousa.example", 11, 5],
  ["Sofia Almeida", "sofia@almeida-advogados.example", 9, 4],
] as const;
const FOUND = [
  ["Joana Reis", "joana@reis-design.example", 6, 1],
  ["João Ferreira", "joao@ferreira-imoveis.example", 4, 0],
  ["Billing", "billing@cloudhost.example", 31, 0],
] as const;
const sender = ([name, address, received, replied]: readonly [string, string, number, number]) => ({
  name,
  address,
  received,
  replied,
});

/** `list_senders`: the suggestions, or everyone whose name or address has every word of `q`. */
const senders: MockRoute = (_body, url) => {
  const q = url.searchParams.get("q");
  if (q === null) {
    return { body: { senders: SUGGESTED.map(sender) } };
  }
  const fold = (text: string) =>
    text
      .normalize("NFD")
      .replaceAll(/\p{Diacritic}/gu, "")
      .toLowerCase();
  const words = fold(q).split(/\s+/u);
  const all = [...SUGGESTED, ...FOUND].map(sender);
  return {
    body: {
      senders: all
        .filter((each) =>
          words.every((word) => fold(`${each.address} ${each.name}`).includes(word)),
        )
        .slice(0, 10),
    },
  };
};

const signedIn = (extra: Record<string, MockRoute> = {}): Record<string, MockRoute> => ({
  "GET /bff/session": { body: SESSION },
  "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
  [`GET ${W}/inboxes`]: { body: { inboxes: [{ ...INBOX, shared: false }] } },
  [`GET ${W}/assistant/profile`]: { body: PROFILE },
  [`GET ${W}/senders`]: senders,
  [`POST ${W}/inboxes/${INBOX.id}/rules/preview`]: {
    body: {
      matching: 6,
      would_change: 6,
      samples: [],
      skipped_suspicious: 0,
      skipped_member: 0,
      overrides: [],
    },
  },
  ...extra,
});

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
  title: "Screens/Scout's questions",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The first view: the ten senders replied to most, and the filter box. */
export const AlwaysReachYou: Story = {
  args: { path: PAGE, api: signedIn() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { name: "Whose mail should always reach you?" }),
    ).toBeVisible();
    await expect(await canvas.findByRole("checkbox", { name: "Maya Okafor" })).toBeInTheDocument();
    await expect(canvas.getByRole("searchbox", { name: "Find someone" })).toBeVisible();
  },
};

/** "joana" finds Joana Reis, who isn't among the ten; Maya stays ticked. */
export const FindByName: Story = {
  args: { path: PAGE, api: signedIn() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("checkbox", { name: "Maya Okafor" }));
    await userEvent.type(canvas.getByRole("searchbox", { name: "Find someone" }), "joana");
    await userEvent.click(await canvas.findByRole("checkbox", { name: "Joana Reis" }));
    await expect(canvas.getByRole("checkbox", { name: "Maya Okafor" })).toBeChecked();
    await expect(canvas.queryByRole("checkbox", { name: "Rita Gomes" })).toBeNull();
    await expect(
      await canvas.findByText(
        "Mail from Joana Reis (joana@reis-design.example) always stays in your inbox and never gets filed away.",
      ),
    ).toBeVisible();
  },
};

/** An address no one in the mail matches, as its own card. */
export const SomeoneNew: Story = {
  args: { path: PAGE, api: signedIn() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(
      await canvas.findByRole("searchbox", { name: "Find someone" }),
      "new@studio.example",
    );
    await expect(await canvas.findByText("No mail from them yet")).toBeVisible();
  },
};
