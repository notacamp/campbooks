import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  ALL_PROVIDERS,
  CONNECTION,
  GOOGLE_WAY_IN,
  LAPTOP_PASSKEY,
  PHONE_PASSKEY,
  ZOHO_WAY_IN,
} from "../../test/demo/identity";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";
import { CREATION_OPTIONS, mockWebAuthn, type MockWebAuthn } from "./mock-webauthn";

// Settings → Ways to sign in, in the real app (router, session gate,
// workspace frame) against in-memory demo data: every state, tested with axe
// in light and dark. Nothing leaves the page: no story follows the front door.

const problem = (status: number, code: string): MockRoute => ({
  status,
  body: { type: "about:blank", title: "Problem", status, code },
});

const PAGE = `/w/${DUARTE.id}/settings/sign-in`;

/** Signed in to Duarte & Pessoa, with `items` as ways in. */
function signedIn({
  items = [GOOGLE_WAY_IN, ZOHO_WAY_IN],
  providers = ALL_PROVIDERS,
  connections = [],
  link,
  remove,
  ways,
  passkeys = [LAPTOP_PASSKEY],
  addPasskey,
  removePasskey,
  everywhere,
}: {
  items?: readonly unknown[];
  providers?: readonly string[];
  connections?: readonly unknown[];
  link?: MockRoute;
  remove?: MockRoute;
  ways?: MockRoute;
  passkeys?: readonly unknown[];
  addPasskey?: MockRoute;
  removePasskey?: MockRoute;
  everywhere?: MockRoute;
}): Record<string, MockRoute> {
  const routes: Record<string, MockRoute> = {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    "GET /bff/api/v1/identity/ways-in": ways ?? { body: { items, providers } },
    [`GET /bff/api/v1/w/${DUARTE.id}/connections`]: { body: { connections } },
    "GET /bff/api/v1/identity/passkeys": { body: { items: passkeys } },
    "POST /bff/api/v1/identity/passkeys/registrations": addPasskey ?? {
      status: 201,
      body: CREATION_OPTIONS,
    },
    "POST /bff/api/v1/identity/passkeys": {
      status: 201,
      body: { ...PHONE_PASSKEY, id: "p-new", name: "Passkey on Mac", synced: true },
    },
  };
  if (removePasskey !== undefined) {
    for (const passkey of [LAPTOP_PASSKEY, PHONE_PASSKEY]) {
      routes[`DELETE /bff/api/v1/identity/passkeys/${passkey.id}`] = removePasskey;
    }
  }
  if (link !== undefined) {
    routes["POST /bff/api/v1/identity/links"] = link;
  }
  if (everywhere !== undefined) {
    routes["POST /bff/api/v1/identity/sign-out-everywhere"] = everywhere;
  }
  if (remove !== undefined) {
    for (const way of [GOOGLE_WAY_IN, ZOHO_WAY_IN]) {
      routes[`DELETE /bff/api/v1/identity/ways-in/${way.id}`] = remove;
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
  /** A pretend browser with passkeys. */
  webauthn?: MockWebAuthn;
}

const meta = {
  title: "Screens/Ways to sign in",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    const restoreFetch = mockFetch(args.api);
    const restoreWebAuthn = mockWebAuthn(args.webauthn ?? {});
    return () => {
      restoreFetch();
      restoreWebAuthn();
    };
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Signed up with Google, then added Zoho; Microsoft is on offer too. */
export const TwoWaysIn: Story = {
  args: { path: PAGE, api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Signed up with")).toBeVisible();
    const add = canvas.getByRole("button", { name: /Add a Google account/u });
    await expect(add).toHaveAttribute("aria-keyshortcuts", "A");
  },
};

/** Remove, confirmed in place. */
export const ConfirmRemove: Story = {
  args: { path: PAGE, api: signedIn({ remove: { status: 204 } }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Remove Zoho (ana.duarte@example.net)" }),
    );
    await expect(canvas.getByText(/stays connected/u)).toBeVisible();
  },
};

/** The only way in: removing it is refused and explained. */
export const LastWayIn: Story = {
  args: {
    path: PAGE,
    api: signedIn({ items: [GOOGLE_WAY_IN], remove: problem(409, "last_way_in") }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /^Remove Google/u }));
    await userEvent.click(canvas.getByRole("button", { name: "Remove" }));
    await expect(await canvas.findByText("This is your only way to sign in.")).toBeVisible();
  },
};

/** The last sign-in is too old to change how you sign in. */
export const SignInAgainFirst: Story = {
  args: { path: PAGE, api: signedIn({ link: problem(403, "step_up_required") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Add a Microsoft account/u }));
    await expect(await canvas.findByText("Sign in again first")).toBeVisible();
  },
};

/** A mailbox of yours here stopped syncing: Reconnect, on its way in. */
export const NeedsReconnect: Story = {
  args: {
    path: PAGE,
    api: signedIn({ connections: [{ ...CONNECTION, health: "needs_reauth" }] }),
  },
};

/** Back from the front door: a way in was added. */
export const Linked: Story = {
  args: { path: `${PAGE}?linked=zoho`, api: signedIn({}) },
};

/** Back from the front door: the account is another Campbooks account's. */
export const BelongsToAnotherAccount: Story = {
  args: {
    path: `${PAGE}?link_error=way_in_belongs_to_another_account`,
    api: signedIn({ items: [GOOGLE_WAY_IN] }),
  },
};

/** Back from the front door: mail access was unticked. */
export const MailNotAllowed: Story = {
  args: { path: `${PAGE}?link_error=consent_incomplete`, api: signedIn({}) },
};

/** No sign-in provider on this instance. */
export const NoProviders: Story = {
  args: { path: PAGE, api: signedIn({ items: [GOOGLE_WAY_IN], providers: [] }) },
};

/** The list didn't load. */
export const LoadFailed: Story = {
  args: { path: PAGE, api: signedIn({ ways: { status: 503 } }) },
};

/** Two days ago, as the story runs. */
const twoDaysAgo = () => new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString();

/** A way in added this week (by someone else?): marked New, and what to do. */
export const RecentlyAdded: Story = {
  args: {
    path: PAGE,
    api: signedIn({
      items: [GOOGLE_WAY_IN, { ...ZOHO_WAY_IN, linked_at: twoDaysAgo() }],
      passkeys: [LAPTOP_PASSKEY],
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("A way to sign in was added this week")).toBeVisible();
    await expect(canvas.getByText("New")).toBeVisible();
  },
};

/** Sign out everywhere, confirmed in place. */
export const SignOutEverywhere: Story = {
  args: { path: PAGE, api: signedIn({ everywhere: { status: 204 } }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Sign out everywhere" }));
    await expect(canvas.getByText(/this browser included/u)).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Keep it" })).toHaveFocus();
  },
};

/** Signing out everywhere failed: still signed in, told so. */
export const SignOutEverywhereFailed: Story = {
  args: { path: PAGE, api: signedIn({ everywhere: problem(503, "unavailable") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Sign out everywhere" }));
    await userEvent.click(canvas.getByRole("button", { name: "Yes, sign out everywhere" }));
    await expect(await canvas.findByText(/Couldn’t sign out everywhere/u)).toBeVisible();
  },
};

/** Sign out everywhere, in Spanish, dark. */
export const SignOutEverywhereSpanish: Story = {
  args: { path: PAGE, api: signedIn({ everywhere: { status: 204 } }) },
  globals: { locale: "es-ES", theme: "dark" },
};

/** In the personal accent. */
export const PersonalAccent: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { accent: "personal" },
};

/** In the forest accent. */
export const ForestAccent: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { accent: "forest" },
};

/** In Portuguese. */
export const Portuguese: Story = {
  args: { path: PAGE, api: signedIn({}) },
  globals: { locale: "pt-PT" },
};

// --- Passkeys ----------------------------------------------------------------

/** Passkeys below the provider accounts: a synced one and one on a phone. */
export const Passkeys: Story = {
  args: { path: PAGE, api: signedIn({ passkeys: [LAPTOP_PASSKEY, PHONE_PASSKEY] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const list = await canvas.findByRole("list", { name: "Your passkeys" });
    await expect(within(list).getByText("Passkey on Mac")).toBeVisible();
    await expect(within(list).getByText(/Synced across your devices/u)).toBeVisible();
    await expect(canvas.getByRole("button", { name: /Add a passkey/u })).toBeVisible();
  },
};

/** No passkey yet. */
export const NoPasskeys: Story = {
  args: { path: PAGE, api: signedIn({ passkeys: [] }) },
};

/** Add a passkey: the browser makes one, and it is listed. */
export const AddPasskey: Story = {
  args: { path: PAGE, api: signedIn({ passkeys: [] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Add a passkey/u }));
    await expect(await canvas.findByText(/Passkey on Mac is added/u)).toBeVisible();
  },
};

/** Adding one needs a recent sign-in. */
export const AddPasskeySignInAgain: Story = {
  args: { path: PAGE, api: signedIn({ addPasskey: problem(403, "step_up_required") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Add a passkey/u }));
    await expect(await canvas.findByText("Sign in again first")).toBeVisible();
  },
};

/** Adding one needs a sign-in from the last five minutes: sign in now. */
export const AddPasskeySignInNow: Story = {
  args: { path: PAGE, api: signedIn({ addPasskey: problem(403, "fresh_sign_in_required") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Add a passkey/u }));
    await expect(await canvas.findByText(/the last five minutes/u)).toBeVisible();
  },
};

/** The passkey dialog was closed. */
export const AddPasskeyCancelled: Story = {
  args: { path: PAGE, api: signedIn({}), webauthn: { ceremony: "cancel" } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Add a passkey/u }));
    await expect(await canvas.findByText("No passkey was added.")).toBeVisible();
  },
};

/** The passkey is the only way in: removing it is refused and explained. */
export const PasskeyLastWayIn: Story = {
  args: {
    path: PAGE,
    api: signedIn({ items: [], removePasskey: problem(409, "last_way_in") }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Remove Passkey on Mac" }));
    await userEvent.click(canvas.getByRole("button", { name: "Remove" }));
    await expect(await canvas.findByText("This is your only way to sign in.")).toBeVisible();
  },
};

/** Rename, in place. */
export const RenamePasskey: Story = {
  args: { path: PAGE, api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Rename Passkey on Mac" }));
    await expect(canvas.getByRole("textbox", { name: "Name" })).toHaveFocus();
  },
};

/** Passkeys in French, dark. */
export const PasskeysFrench: Story = {
  args: { path: PAGE, api: signedIn({ passkeys: [LAPTOP_PASSKEY, PHONE_PASSKEY] }) },
  globals: { locale: "fr", theme: "dark" },
};
