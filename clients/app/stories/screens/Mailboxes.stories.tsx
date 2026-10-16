import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  CONNECTION,
  GOOGLE_WAY_IN,
  SHARED_CONNECTION,
  SOFIA_CONNECTION,
  ZOHO_CONNECTION,
  ZOHO_WAY_IN,
} from "../../test/demo/identity";
import { ANA, DUARTE, HOUSEHOLD, SESSION, SOFIA } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Settings → Mailboxes, in the real app (router, session gate, workspace
// frame) against in-memory demo data: every state, tested with axe in light
// and dark. Nothing leaves the page: no story follows the front door.

const problem = (status: number, code: string): MockRoute => ({
  status,
  body: { type: "about:blank", title: "Problem", status, code },
});

const PAGE = `/w/${DUARTE.id}/settings/mailboxes`;

/** Signed in to Duarte & Pessoa (as its owner), with `connections`. */
function signedIn({
  workspace = DUARTE,
  connections = [CONNECTION, ZOHO_CONNECTION, SOFIA_CONNECTION],
  mailboxProviders = ["google", "zoho"],
  list,
  link,
  disconnect,
}: {
  workspace?: typeof DUARTE | typeof HOUSEHOLD;
  connections?: readonly unknown[];
  mailboxProviders?: readonly string[];
  list?: MockRoute;
  link?: MockRoute;
  disconnect?: MockRoute;
}): Record<string, MockRoute> {
  const routes: Record<string, MockRoute> = {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [workspace] } },
    "GET /bff/api/v1/identity/held-grants": { body: { items: [] } },
    "GET /bff/api/v1/identity/passkeys": { body: { items: [] } },
    "GET /bff/api/v1/identity/ways-in": {
      body: {
        // Ana signed up with Google, and added Zoho by connecting its mailbox.
        items: [
          GOOGLE_WAY_IN,
          {
            ...ZOHO_WAY_IN,
            linked_via: "connect",
            mailboxes: 1,
            connections: [ZOHO_CONNECTION.id],
          },
        ],
        providers: ["google", "microsoft", "zoho"],
        mailbox_providers: mailboxProviders,
      },
    },
    [`GET /bff/api/v1/w/${workspace.id}/connections`]: list ?? { body: { connections } },
    [`GET /bff/api/v1/w/${workspace.id}/members`]: { body: { members: [ANA, SOFIA] } },
  };
  if (link !== undefined) {
    routes["POST /bff/api/v1/identity/links"] = link;
  }
  if (disconnect !== undefined) {
    for (const connection of [CONNECTION, ZOHO_CONNECTION, SOFIA_CONNECTION, SHARED_CONNECTION]) {
      routes[`DELETE /bff/api/v1/w/${workspace.id}/connections/${connection.id}`] = disconnect;
    }
  }
  return routes;
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
  title: "Screens/Mailboxes",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Gmail syncing, Zoho Mail waiting for Ana, Sofia's Gmail paused by Gmail. */
export const EveryHealth: Story = {
  args: { path: PAGE, api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Needs you to sign in again")).toBeVisible();
    await expect(canvas.getByText("Paused by Gmail for a few minutes")).toBeVisible();
    const connect = canvas.getByRole("button", { name: /Connect Gmail/u });
    await expect(connect).toHaveAttribute("aria-keyshortcuts", "Shift+C");
    const reconnect = canvas.getByRole("button", { name: "Reconnect ana.duarte@example.net" });
    await expect(reconnect).toHaveAttribute("aria-keyshortcuts", "R");
  },
};

/** One mailbox that keeps failing: "we'll keep trying". */
export const Failing: Story = {
  args: {
    path: PAGE,
    api: signedIn({ connections: [{ ...CONNECTION, health: "failing" }] }),
  },
};

/** A colleague's mailbox needs her: named, and no Reconnect for you. */
export const ColleagueNeedsToSignIn: Story = {
  args: {
    path: PAGE,
    api: signedIn({ connections: [CONNECTION, { ...SOFIA_CONNECTION, health: "needs_reauth" }] }),
  },
};

/**
 * Disconnect, confirmed in place: delete its mail (the default) or keep its
 * history; the mail stays in Gmail, and so does the way in.
 */
export const ConfirmDisconnect: Story = {
  args: { path: PAGE, api: signedIn({ disconnect: { status: 204 } }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Disconnect ana@example.com" }),
    );
    await expect(
      canvas.getByRole("radio", { name: "Delete its mail from Campbooks" }),
    ).toBeChecked();
    await expect(canvas.getByText(/How you sign in to Campbooks doesn’t change/u)).toBeVisible();
  },
};

/** Keep its history, read-only, chosen. */
export const KeepHistory: Story = {
  args: { path: PAGE, api: signedIn({ disconnect: { status: 204 } }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Disconnect ana@example.com" }),
    );
    await userEvent.click(canvas.getByRole("radio", { name: "Keep its history, read-only" }));
    await userEvent.click(canvas.getByRole("button", { name: "Disconnect" }));
    await expect(await canvas.findByText(/Its history stays here, read-only/u)).toBeVisible();
  },
};

/** Zoho was added only by connecting it: disconnecting it removes that way in too. */
export const WayInGoesWithIt: Story = {
  args: {
    path: PAGE,
    api: signedIn({
      connections: [CONNECTION, { ...ZOHO_CONNECTION, health: "ok" }],
      disconnect: { status: 204 },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Disconnect ana.duarte@example.net" }),
    );
    await expect(canvas.getByText(/also stops being a way to sign in/u)).toBeVisible();
  },
};

/** A shared mailbox: not a way in, so how you sign in doesn’t change. */
export const SharedMailbox: Story = {
  args: {
    path: `/w/${HOUSEHOLD.id}/settings/mailboxes`,
    api: signedIn({
      workspace: HOUSEHOLD,
      connections: [SHARED_CONNECTION],
      disconnect: { status: 204 },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Disconnect loja@example.com" }),
    );
    await expect(canvas.getByText(/How you sign in to Campbooks doesn’t change/u)).toBeVisible();
  },
};

/** A disconnect that finds nothing is said, never shown as done. */
export const DisconnectNotFound: Story = {
  args: { path: PAGE, api: signedIn({ disconnect: { status: 404 } }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Disconnect ana@example.com" }),
    );
    await userEvent.click(canvas.getByRole("button", { name: "Disconnect" }));
    await expect(await canvas.findByText(/wasn’t found/u)).toBeVisible();
  },
};

/** Connecting a shared mailbox: "Also use this account to sign in" unticked. */
export const ConnectShared: Story = {
  args: { path: PAGE, api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const box = await canvas.findByRole("checkbox", { name: "Also use this account to sign in" });
    await userEvent.click(box);
    await expect(box).not.toBeChecked();
  },
};

/** After Disconnect. */
export const Disconnected: Story = {
  args: { path: PAGE, api: signedIn({ disconnect: { status: 204 } }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Disconnect ana@example.com" }),
    );
    await userEvent.click(canvas.getByRole("button", { name: "Disconnect" }));
    await expect(await canvas.findByText("ana@example.com is disconnected")).toBeVisible();
  },
};

/** The last sign-in is too old to connect a mailbox. */
export const SignInAgainFirst: Story = {
  args: { path: PAGE, api: signedIn({ link: problem(403, "step_up_required") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Connect Zoho Mail/u }));
    await expect(await canvas.findByText("Sign in again first")).toBeVisible();
  },
};

/** Too many tries in a row. */
export const RateLimited: Story = {
  args: { path: PAGE, api: signedIn({ link: problem(429, "rate_limited") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Connect Gmail/u }));
    await expect(await canvas.findByText(/Too many tries/u)).toBeVisible();
  },
};

/** Back from the front door: Zoho Mail connected. */
export const Connected: Story = {
  args: {
    path: `${PAGE}?connected=${ZOHO_CONNECTION.id}`,
    api: signedIn({ connections: [CONNECTION, { ...ZOHO_CONNECTION, health: "ok" }] }),
  },
};

/** Back from the front door: another member connected that mailbox already. */
export const TakenByAnotherMember: Story = {
  args: {
    path: `${PAGE}?link_error=account_connected_by_another_member`,
    api: signedIn({ connections: [CONNECTION] }),
  },
};

/** Back from the front door: mail access was unticked. */
export const MailNotAllowed: Story = {
  args: {
    path: `${PAGE}?link_error=consent_incomplete`,
    api: signedIn({ connections: [CONNECTION] }),
  },
};

/** Back from the front door: reconnecting with another account. */
export const DifferentAccount: Story = {
  args: { path: `${PAGE}?link_error=account_mismatch`, api: signedIn({}) },
};

/** Back from the front door: something this page doesn't know. */
export const UnknownRefusal: Story = {
  args: { path: `${PAGE}?link_error=made_up`, api: signedIn({ connections: [CONNECTION] }) },
};

/** No mailbox yet. */
export const Empty: Story = {
  args: { path: PAGE, api: signedIn({ connections: [] }) },
};

/** No mail provider set up on this instance. */
export const NoProviders: Story = {
  args: { path: PAGE, api: signedIn({ connections: [], mailboxProviders: [] }) },
};

/** The list didn't load. */
export const LoadFailed: Story = {
  args: { path: PAGE, api: signedIn({ list: { status: 503 } }) },
};

/** A member sees only the mailboxes they connected. */
export const Member: Story = {
  args: {
    path: `/w/${HOUSEHOLD.id}/settings/mailboxes`,
    api: signedIn({ workspace: HOUSEHOLD, connections: [CONNECTION] }),
  },
};

/** In the dark. */
export const Dark: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { theme: "dark" },
};

/** In the personal accent. */
export const PersonalAccent: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { accent: "personal" },
};

/** In the forest accent, dark. */
export const ForestAccentDark: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { accent: "forest", theme: "dark" },
};

/** In Portuguese. */
export const Portuguese: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { locale: "pt-PT" },
};

/** In Spanish. */
export const Spanish: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { locale: "es-ES" },
};

/** In French. */
export const French: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { locale: "fr" },
};
