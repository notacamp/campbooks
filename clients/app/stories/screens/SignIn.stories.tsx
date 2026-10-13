import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { writeTabValue } from "~/lib/storage/tab";
import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { CONNECTION, HELD_GRANT, NEW_ACCOUNT, REQUEST } from "../../test/demo/identity";
import { DUARTE, HOUSEHOLD, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// The real app (router, public sign-in routes, session gate, first run, Home)
// against in-memory demo data: every sign-in, new-account and mailbox state,
// tested with axe in light and dark. Nothing here leaves the page: the
// stories never follow a provider link.

const problem = (status: number, code: string): MockRoute => ({
  status,
  body: { type: "about:blank", title: "Problem", status, code },
});

const signedOut: Record<string, MockRoute> = {
  "GET /bff/session": { body: { authenticated: false } },
};

const ways = (providers: string[]): Record<string, MockRoute> => ({
  ...signedOut,
  [`GET /auth/requests/${REQUEST}`]: { body: { client_id: "campbooks-web", providers } },
});

const newAccount = (create?: MockRoute): Record<string, MockRoute> => ({
  ...signedOut,
  "GET /auth/new-account": { body: NEW_ACCOUNT },
  ...(create === undefined ? {} : { "POST /auth/new-account": create }),
});

/** Signed in, with `workspaces`, a held grant or not, and connections. */
function signedIn({
  workspaces,
  held = [],
  connections = [],
}: {
  workspaces: readonly unknown[];
  held?: readonly unknown[];
  connections?: readonly unknown[];
}): Record<string, MockRoute> {
  const list = [...workspaces];
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": () => ({ body: { workspaces: list } }),
    "POST /bff/api/v1/workspaces": (body) => {
      const { kind } = body as { kind?: string };
      const accent = kind === "personal" ? "personal" : "business";
      const created = { ...DUARTE, ...(body as object), accent, role: "owner" };
      list.push(created);
      return { status: 201, body: created };
    },
    "GET /bff/api/v1/identity/held-grants": { body: { items: held } },
    [`GET /bff/api/v1/w/${DUARTE.id}/connections`]: { body: { connections } },
    [`GET /bff/api/v1/w/${HOUSEHOLD.id}/connections`]: { body: { connections } },
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
  /** Where this tab's sign-in comes back to. */
  returnTo?: string;
}

const meta = {
  title: "Screens/Sign in",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    writeTabValue("signIn.returnTo", args.returnTo ?? "");
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

const signInAt = (query: string) => `/sign-in?request=${REQUEST}${query}`;

// --- Sign in ---------------------------------------------------------------

/** What S1 ships: Google only, the one consent explained before leaving. */
export const Google: Story = {
  args: { path: signInAt(""), api: ways(["google"]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const google = await canvas.findByRole("link", { name: /Continue with Google/u });
    await expect(google).toHaveAttribute("aria-keyshortcuts", "1");
    await expect(canvas.getByText(/hasn’t verified this app/u)).toBeVisible();
  },
};

/** "What Campbooks reads and keeps", opened. */
export const GmailDetails: Story = {
  args: { path: signInAt(""), api: ways(["google"]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByText("What Campbooks reads and keeps"));
    await expect(canvas.getByText(/never sees your Google password/u)).toBeVisible();
  },
};

/** Every way in the server can list (S2 and S3 turn them on): 1, 2, 3 and P. */
export const EveryWayIn: Story = {
  args: { path: signInAt(""), api: ways(["google", "microsoft", "zoho", "passkey"]) },
};

/** On the way to an invitation link. */
export const Invited: Story = {
  args: { path: signInAt(""), api: ways(["google"]), returnTo: "/invitations/accept?token=demo" },
};

/** No way in configured on this instance. */
export const NoWayIn: Story = {
  args: { path: signInAt(""), api: ways([]) },
};

/** In the personal accent. */
export const PersonalAccent: Story = {
  args: { path: signInAt(""), api: ways(["google"]) },
  globals: { accent: "personal" },
};

/** In the forest accent. */
export const ForestAccent: Story = {
  args: { path: signInAt(""), api: ways(["google"]) },
  globals: { accent: "forest" },
};

/** In Portuguese. */
export const Portuguese: Story = {
  args: { path: signInAt(""), api: ways(["google"]) },
  globals: { locale: "pt-PT" },
};

// --- Sign-in errors --------------------------------------------------------

/** Cancelled on Google's screen. */
export const CancelledAtProvider: Story = {
  args: { path: signInAt("&error=cancelled_at_provider"), api: ways(["google"]) },
};

/** Google didn't answer. */
export const ProviderUnavailable: Story = {
  args: { path: signInAt("&error=provider_unavailable"), api: ways(["google"]) },
};

/** Google couldn't confirm who it is. */
export const LoginFailed: Story = {
  args: { path: signInAt("&error=login_failed"), api: ways(["google"]) },
};

/** An administrator suspended the account. */
export const AccountSuspended: Story = {
  args: { path: signInAt("&error=account_suspended"), api: ways(["google"]) },
};

/** The provider account is a way into another Campbooks account. */
export const BelongsToAnotherAccount: Story = {
  args: {
    path: signInAt("&error=way_in_belongs_to_another_account"),
    api: ways(["google"]),
  },
};

/** An invite-only instance, and nobody invited this address. */
export const InviteOnly: Story = {
  args: { path: signInAt("&error=signup_invite_only"), api: ways(["google"]) },
};

/** A code this client doesn't know. */
export const UnknownError: Story = {
  args: { path: signInAt("&error=something_new"), api: ways(["google"]) },
};

/** The round trip expired or came back to another browser (no request any more). */
export const StateExpired: Story = {
  args: { path: "/sign-in?error=invalid_state", api: signedOut, returnTo: "/w/demo" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { name: "That sign-in can’t be finished" }),
    ).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Start again" })).toBeVisible();
  },
};

/** The sign-in page waited too long. */
export const RequestExpired: Story = {
  args: {
    path: signInAt(""),
    api: { ...signedOut, [`GET /auth/requests/${REQUEST}`]: { status: 404 } },
  },
};

/** The ways in can't be loaded. */
export const RequestUnreachable: Story = {
  args: {
    path: signInAt(""),
    api: {
      ...signedOut,
      [`GET /auth/requests/${REQUEST}`]: { status: 503, body: { status: 503 } },
    },
  },
};

// --- New account -----------------------------------------------------------

/** No account uses this Google account yet: who it is for, what happens next. */
export const NewAccount: Story = {
  args: { path: `/sign-in/new?request=${REQUEST}`, api: newAccount() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "Create your Campbooks account" }),
    ).toBeVisible();
    await expect(canvas.getByRole("list", { name: "What happens next" })).toBeVisible();
  },
};

/** The provider gave no name or address. */
export const NewAccountNoName: Story = {
  args: {
    path: `/sign-in/new?request=${REQUEST}`,
    api: {
      ...signedOut,
      "GET /auth/new-account": { body: { provider: "google", email: null, display_name: null } },
    },
  },
};

/** New account in the forest accent, dark. */
export const NewAccountForestDark: Story = {
  args: { path: `/sign-in/new?request=${REQUEST}`, api: newAccount() },
  globals: { accent: "forest", theme: "dark" },
};

/** Nothing waits for this browser (expired, or another browser). */
export const NewAccountGone: Story = {
  args: {
    path: `/sign-in/new?request=${REQUEST}`,
    api: { ...signedOut, "GET /auth/new-account": { status: 404 } },
  },
};

/** Another account took this identity meanwhile. */
export const NewAccountTaken: Story = {
  args: {
    path: `/sign-in/new?request=${REQUEST}`,
    api: newAccount(problem(409, "way_in_belongs_to_another_account")),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Create account/u }));
    await expect(await canvas.findByRole("alert")).toBeVisible();
  },
};

/** Invite-only, refused on Create. */
export const NewAccountInviteOnly: Story = {
  args: {
    path: `/sign-in/new?request=${REQUEST}`,
    api: newAccount(problem(403, "signup_invite_only")),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Create account/u }));
    await expect(
      await canvas.findByRole("heading", { name: "This Campbooks is by invitation only" }),
    ).toBeVisible();
  },
};

/** Creating failed (the CSRF check, or the server). */
export const NewAccountFailed: Story = {
  args: {
    path: `/sign-in/new?request=${REQUEST}`,
    api: newAccount(problem(403, "csrf_check_failed")),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /Create account/u }));
    await expect(await canvas.findByRole("alert")).toBeVisible();
  },
};

// --- The mailbox from sign-up ----------------------------------------------

/** Skips both first-run questions, landing on the step after creating. */
async function createWorkspace({ canvasElement }: { canvasElement: HTMLElement }) {
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByRole("button", { name: "Skip" }));
  await userEvent.click(await canvas.findByRole("button", { name: "Skip" }));
  return canvas;
}

/** Right after the first workspace: "Your Gmail … will be connected". */
export const MailboxStep: Story = {
  args: { path: "/welcome", api: signedIn({ workspaces: [], held: [HELD_GRANT] }) },
  play: async (context) => {
    const canvas = await createWorkspace(context);
    await expect(
      await canvas.findByRole("heading", {
        level: 1,
        name: "Your Gmail ana@example.com will be connected to this workspace",
      }),
    ).toBeVisible();
  },
};

/** The step in a personal workspace's accent. */
export const MailboxStepPersonal: Story = {
  args: { path: "/welcome", api: signedIn({ workspaces: [], held: [HELD_GRANT] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Skip" }));
    await userEvent.click(await canvas.findByRole("radio", { name: "Personal" }));
    await userEvent.click(canvas.getByRole("button", { name: "Create workspace" }));
    await canvas.findByRole("button", { name: /Not now/u });
  },
};

/** Another member already connected this mailbox here. */
export const MailboxStepTaken: Story = {
  args: {
    path: "/welcome",
    api: {
      ...signedIn({ workspaces: [], held: [HELD_GRANT] }),
      [`POST /bff/api/v1/w/${DUARTE.id}/identity/held-grants/${HELD_GRANT.id}/adopt`]: problem(
        409,
        "account_connected_by_another_member",
      ),
    },
  },
  play: async (context) => {
    const canvas = await createWorkspace(context);
    await userEvent.click(await canvas.findByRole("button", { name: /^Connect/u }));
    await expect(await canvas.findByRole("alert")).toBeVisible();
  },
};

/** Nothing held (mail permission unticked): the usual "is ready". */
export const NothingHeld: Story = {
  args: { path: "/welcome", api: signedIn({ workspaces: [] }) },
  play: async (context) => {
    const canvas = await createWorkspace(context);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "My workspace is ready" }),
    ).toBeVisible();
  },
};

/** After Not now: Home keeps offering it until it expires. */
export const HomeOffer: Story = {
  args: { path: `/w/${DUARTE.id}`, api: signedIn({ workspaces: [DUARTE], held: [HELD_GRANT] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { name: "Connect ana@example.com" }),
    ).toBeVisible();
  },
};

/** Forgetting it is confirmed in place. */
export const HomeOfferForget: Story = {
  args: { path: `/w/${DUARTE.id}`, api: signedIn({ workspaces: [DUARTE], held: [HELD_GRANT] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Don’t connect it" }));
    await expect(canvas.getByRole("button", { name: "Forget it" })).toBeVisible();
  },
};

/** Once connected, Home names the mailbox. */
export const HomeConnected: Story = {
  args: {
    path: `/w/${DUARTE.id}`,
    api: signedIn({ workspaces: [DUARTE], connections: [CONNECTION] }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { name: "Your mail" })).toBeVisible();
  },
};
