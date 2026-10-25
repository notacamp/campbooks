import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  CONNECTION,
  GMX_DISCOVERY,
  GOOGLE_WAY_IN,
  IMAP_CONNECTION,
  SHARED_CONNECTION,
  SOFIA_CONNECTION,
  ZOHO_CONNECTION,
  ZOHO_WAY_IN,
  YAHOO_DISCOVERY,
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
const ADD = `${PAGE}?add=1`;
const W = `/bff/api/v1/w/${DUARTE.id}`;

/** Discovery as the server answers it, by the address's domain (demo domains only). */
function discovery(body: unknown) {
  const address = (body as { address?: string } | undefined)?.address ?? "";
  const domain = address.split("@").at(-1) ?? "";
  if (domain === "gmail.com") {
    return { body: { kind: "use_provider", provider: "google" } };
  }
  if (domain === "zohomail.com") {
    return { body: { kind: "use_provider", provider: "zoho" } };
  }
  if (domain === "outlook.com") {
    return { body: { kind: "not_supported_yet", provider: "microsoft" } };
  }
  if (domain.startsWith("gmx")) {
    return { body: { ...GMX_DISCOVERY, username_hint: address } };
  }
  if (domain.startsWith("yahoo")) {
    return { body: { ...YAHOO_DISCOVERY, username_hint: address } };
  }
  return { body: { kind: "not_found" } };
}

/**
 * Add a mailbox's server side, kept in memory: discovery by domain; a
 * connect that `refuse`s with a code, or creates the bakery's mailbox, its
 * address being checked; its check that ends `verified` (or `failed`)
 * after `checks` reads, and a retry that starts it again.
 */
function addMailbox({
  refuse,
  ends = "verified",
  checks = 2,
}: { refuse?: string; ends?: "verified" | "failed"; checks?: number } = {}) {
  let reads = 0;
  let outcome = ends;
  let connected: typeof IMAP_CONNECTION | undefined;
  const now = () => {
    if (connected === undefined) {
      return IMAP_CONNECTION;
    }
    reads += 1;
    if (reads <= checks) {
      return connected;
    }
    return outcome === "verified"
      ? { ...connected, address_verified: true, address_check: null }
      : { ...connected, address_check: "failed" as const };
  };
  return {
    [`POST ${W}/connections/imap/discover`]: (body: unknown) => discovery(body),
    [`POST ${W}/connections/imap`]: (body: unknown) => {
      if (refuse !== undefined) {
        return {
          status: 400,
          body: { type: "about:blank", title: "Problem", status: 400, code: refuse },
        };
      }
      const address = (body as { address?: string } | undefined)?.address ?? "";
      connected = { ...IMAP_CONNECTION, address };
      reads = 0;
      return { status: 201, body: connected };
    },
    [`GET ${W}/connections/${IMAP_CONNECTION.id}`]: () => ({ body: now() }),
    [`POST ${W}/connections/${IMAP_CONNECTION.id}/address-check`]: () => {
      reads = 0;
      outcome = "verified";
      return { status: 202, body: connected ?? IMAP_CONNECTION };
    },
  } satisfies Record<string, MockRoute>;
}

/** Type `address` in the open Add a mailbox, and continue. */
async function addAddress(canvasElement: HTMLElement, address: string) {
  const dialog = within(
    await within(canvasElement.ownerDocument.body).findByRole("dialog", { name: "Add a mailbox" }),
  );
  const field = await dialog.findByLabelText("Email address");
  await userEvent.type(field, address);
  await userEvent.click(dialog.getByRole("button", { name: /Continue/u }));
  return dialog;
}

/** Signed in to Duarte & Pessoa (as its owner), with `connections`. */
function signedIn({
  workspace = DUARTE,
  connections = [CONNECTION, ZOHO_CONNECTION, SOFIA_CONNECTION],
  mailboxProviders = ["google", "zoho"],
  list,
  link,
  disconnect,
  imap,
}: {
  workspace?: typeof DUARTE | typeof HOUSEHOLD;
  connections?: readonly unknown[];
  mailboxProviders?: readonly string[];
  list?: MockRoute;
  link?: MockRoute;
  disconnect?: MockRoute;
  /** Add a mailbox's server side (`addMailbox`). */
  imap?: Record<string, MockRoute>;
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
  if (imap !== undefined) {
    Object.assign(routes, imap);
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
    const connect = canvas.getByRole("button", { name: /Add a mailbox/u });
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
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "loja@gmail.com");
    const box = await dialog.findByRole("checkbox", { name: "Also use this account to sign in" });
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
  args: {
    path: ADD,
    api: signedIn({ link: problem(403, "step_up_required"), imap: addMailbox() }),
  },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@zohomail.com");
    await userEvent.click(await dialog.findByRole("button", { name: /Connect Zoho Mail/u }));
    await expect(await dialog.findByText("Sign in again first")).toBeVisible();
  },
};

/** Too many tries in a row. */
export const RateLimited: Story = {
  args: { path: ADD, api: signedIn({ link: problem(429, "rate_limited"), imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@gmail.com");
    await userEvent.click(await dialog.findByRole("button", { name: /Connect Gmail/u }));
    await expect(await dialog.findByText(/Too many tries/u)).toBeVisible();
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

/* Add a mailbox (campbooks-cloud#314): the address first, then the way it
   connects. Each step has a story that reaches it, and a twin without a
   play for the recordings (375px and desktop), which act themselves. */

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

/** Add a mailbox, open: one address field. */
export const AddAddress: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = within(canvasElement.ownerDocument.body);
    await expect(await dialog.findByLabelText("Email address")).toHaveFocus();
    await expect(dialog.getByRole("button", { name: /Continue/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "Enter",
    );
  },
};

/** A Gmail address: Google's own sign-in, no password. */
export const AddGmail: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@gmail.com");
    await expect(await dialog.findByRole("button", { name: /Connect Gmail/u })).toBeVisible();
  },
};

/** An Outlook address: coming soon. */
export const AddOutlook: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@outlook.com");
    await expect(await dialog.findByText("Outlook is coming soon")).toBeVisible();
  },
};

/** A Yahoo address: Yahoo's app-password steps, one password field. */
export const AddAppPassword: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@yahoo.example");
    await expect(
      await dialog.findByRole("list", { name: "How to get an app password at Yahoo" }),
    ).toBeVisible();
    await expect(dialog.getByLabelText("App password")).toHaveFocus();
  },
};

/** A GMX address: turn on IMAP first. */
export const AddGmx: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@gmx.example");
    await expect(await dialog.findByText(/Turn on IMAP first/u)).toBeVisible();
  },
};

/** Advanced open: the username and both servers, from discovery. */
export const AddAdvanced: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox() }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@yahoo.example");
    await userEvent.click(await dialog.findByText("Advanced: username and servers"));
    await expect(dialog.getAllByLabelText("Server")[0]).toBeVisible();
  },
};

/** The app password refused: explained, the field empty. */
export const AddRefused: Story = {
  args: { path: ADD, api: signedIn({ imap: addMailbox({ refuse: "imap_login_refused" }) }) },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@yahoo.example");
    await userEvent.type(await dialog.findByLabelText("App password"), "abcd efgh ijkl mnop");
    await userEvent.click(dialog.getByRole("button", { name: /^Connect/u }));
    await expect(await dialog.findByRole("alert")).toHaveTextContent(
      /Yahoo needs an app password/u,
    );
    await expect(dialog.getByLabelText("App password")).toHaveValue("");
  },
};

/** Connected; its address is being checked. */
export const AddVerifying: Story = {
  args: {
    path: ADD,
    api: signedIn({ connections: [CONNECTION], imap: addMailbox({ checks: 1000 }) }),
  },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@bakery.example");
    for (const host of await dialog.findAllByLabelText("Server")) {
      await userEvent.type(host, "mail.bakery.example");
    }
    await userEvent.type(dialog.getByLabelText("Password"), "app-password");
    await userEvent.click(dialog.getByRole("button", { name: /^Connect/u }));
    await expect(await dialog.findByText("Checking your address…")).toBeVisible();
  },
};

/** The address check found its code: verified. */
export const AddVerified: Story = {
  args: {
    path: ADD,
    api: signedIn({ connections: [CONNECTION], imap: addMailbox({ checks: 0 }) }),
  },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@yahoo.example");
    await userEvent.type(await dialog.findByLabelText("App password"), "abcd efgh ijkl mnop");
    await userEvent.click(dialog.getByRole("button", { name: /^Connect/u }));
    await expect(await dialog.findByText("Verified")).toBeVisible();
  },
};

/** The check did not find its code: sending stays off, Try again. */
export const AddNotVerified: Story = {
  args: {
    path: ADD,
    api: signedIn({ connections: [CONNECTION], imap: addMailbox({ checks: 0, ends: "failed" }) }),
  },
  play: async ({ canvasElement }) => {
    const dialog = await addAddress(canvasElement, "ana@yahoo.example");
    await userEvent.type(await dialog.findByLabelText("App password"), "abcd efgh ijkl mnop");
    await userEvent.click(dialog.getByRole("button", { name: /^Connect/u }));
    await expect(
      await dialog.findByText("We couldn't verify this address: sending stays off"),
    ).toBeVisible();
  },
};

/** A mailbox whose address is being checked, and one whose check failed, in the list. */
export const AddressChecks: Story = {
  args: {
    path: PAGE,
    api: signedIn({
      connections: [
        CONNECTION,
        IMAP_CONNECTION,
        {
          ...IMAP_CONNECTION,
          id: "0192f1a0-0000-7000-8000-0000000000c6",
          address: "contas@bakery.example",
          address_check: "failed",
        },
      ],
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Checking your address…")).toBeVisible();
    await expect(
      canvas.getByRole("button", { name: "Check contas@bakery.example again" }),
    ).toBeVisible();
  },
};

/** Add a mailbox at 375px. */
export const AddAppPasswordPhone: Story = {
  ...AddAppPassword,
  ...at("phone", 375, 812),
};

/** Add a mailbox, to drive by hand: discovery by domain, connect, a check that verifies. */
export const AddMailboxIdle: Story = {
  args: { path: ADD, api: signedIn({ connections: [CONNECTION], imap: addMailbox() }) },
};

/** The same at 375px. */
export const AddMailboxIdlePhone: Story = {
  ...AddMailboxIdle,
  ...at("phone", 375, 812),
};

/** Add a mailbox whose password is refused, to drive by hand. */
export const AddRefusedIdle: Story = {
  args: {
    path: ADD,
    api: signedIn({
      connections: [CONNECTION],
      imap: addMailbox({ refuse: "imap_login_refused" }),
    }),
  },
};

/** Add a mailbox whose check fails, then passes on Try again, to drive by hand. */
export const AddNotVerifiedIdle: Story = {
  args: {
    path: ADD,
    api: signedIn({ connections: [CONNECTION], imap: addMailbox({ ends: "failed" }) }),
  },
};

/** The same at 375px. */
export const AddNotVerifiedIdlePhone: Story = {
  ...AddNotVerifiedIdle,
  ...at("phone", 375, 812),
};
