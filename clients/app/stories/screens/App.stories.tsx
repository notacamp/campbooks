import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  ANA,
  DUARTE,
  EXPIRED,
  HOUSEHOLD,
  PENDING,
  RUI,
  SESSION,
  SOFIA,
} from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// The real app (router, session gate, shell, screens) against in-memory demo
// data: every new first-run screen, tested with axe in light and dark.

const signedIn = (workspaces: readonly unknown[]): Record<string, MockRoute> => ({
  "GET /bff/session": { body: SESSION },
  "GET /bff/api/v1/workspaces": { body: { workspaces } },
  [`GET /bff/api/v1/w/${DUARTE.id}/members`]: { body: { members: [ANA, SOFIA, RUI] } },
  [`GET /bff/api/v1/w/${DUARTE.id}/invitations`]: {
    body: { invitations: [PENDING, EXPIRED] },
  },
  "POST /bff/api/v1/workspaces": (body) => ({
    status: 201,
    body: { ...HOUSEHOLD, ...(body as object), role: "owner" },
  }),
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
  title: "Screens/First run",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

// Signed out, the app goes to the sign-in screen (which leaves for the BFF):
// its states are in "Screens/Sign in".

/** The server can't be reached. */
export const Unreachable: Story = {
  args: { path: "/", api: { "GET /bff/session": { status: 503, body: { status: 503 } } } },
};

/** No workspace yet: name, then kind (with its accent), then done. */
export const CreateFirstWorkspace: Story = {
  args: { path: "/", api: signedIn([]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const name = await canvas.findByRole("textbox", { name: "Workspace name" });
    await userEvent.type(name, "Household");
    await userEvent.keyboard("{Enter}");
    await userEvent.click(await canvas.findByRole("radio", { name: "Personal" }));
    await userEvent.click(canvas.getByRole("button", { name: "Create workspace" }));
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "Household is ready" }),
    ).toBeVisible();
  },
};

/** Skips both questions, so the create call meets the signup policy. */
async function skipBoth({ canvasElement }: { canvasElement: HTMLElement }) {
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByRole("button", { name: "Skip" }));
  await userEvent.click(await canvas.findByRole("button", { name: "Skip" }));
}

/** The instance asks for a signup code. */
export const SignupCode: Story = {
  args: {
    path: "/welcome",
    api: {
      ...signedIn([]),
      "POST /bff/api/v1/workspaces": {
        status: 403,
        body: { status: 403, code: "signup_code_invalid", title: "Forbidden" },
      },
    },
  },
  play: async ({ canvasElement }) => {
    await skipBoth({ canvasElement });
    await expect(
      await within(canvasElement).findByRole("textbox", { name: "Signup code" }),
    ).toBeVisible();
  },
};

/** The instance only lets invited people in. */
export const InviteOnly: Story = {
  args: {
    path: "/welcome",
    api: {
      ...signedIn([]),
      "POST /bff/api/v1/workspaces": {
        status: 403,
        body: { status: 403, code: "signup_invite_only", title: "Forbidden" },
      },
    },
  },
  play: async ({ canvasElement }) => {
    await skipBoth({ canvasElement });
    await expect(
      await within(canvasElement).findByRole("heading", { name: "You need an invitation" }),
    ).toBeVisible();
  },
};

/** A workspace's Home before mail is connected. */
export const EmptyHome: Story = {
  args: { path: `/w/${DUARTE.id}`, api: signedIn([DUARTE, HOUSEHOLD]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "Nothing needs you yet" }),
    ).toBeVisible();
  },
};

/** ⌘⇧W: every workspace in its own accent. */
export const SwitchWorkspace: Story = {
  args: { path: `/w/${DUARTE.id}`, api: signedIn([DUARTE, HOUSEHOLD]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 1, name: "Nothing needs you yet" });
    await userEvent.click(canvas.getByRole("button", { name: /Switch workspace/u }));
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByRole("dialog", { name: "Switch workspace" })).toBeVisible();
  },
};

/** The account menu: theme, density, language, sign out. */
export const AccountMenu: Story = {
  args: { path: `/w/${DUARTE.id}`, api: signedIn([DUARTE]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 1, name: "Nothing needs you yet" });
    await userEvent.click(canvas.getByRole("button", { name: /Ana Duarte/u }));
    await expect(await canvas.findByRole("dialog", { name: "Account" })).toBeVisible();
  },
};

/** Settings → Members, as an owner: roles, removal, invitations. */
export const Members: Story = {
  args: { path: `/w/${DUARTE.id}/settings/members`, api: signedIn([DUARTE]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Rui Matos");
    await userEvent.click(canvas.getByRole("button", { name: "Remove Rui Matos" }));
    await waitFor(() =>
      expect(canvas.getByRole("group", { name: /Remove Rui Matos/u })).toBeVisible(),
    );
  },
};

/** Inviting someone shows the accept link once, to copy and send on. */
export const InviteWithLink: Story = {
  args: {
    path: `/w/${DUARTE.id}/settings/members`,
    api: {
      ...signedIn([DUARTE]),
      [`POST /bff/api/v1/w/${DUARTE.id}/invitations`]: (body) => ({
        status: 201,
        body: {
          ...PENDING,
          id: "new",
          email: (body as { email: string }).email,
          accept_url: "https://campbooks.example.com/invitations/accept?token=demo-not-real",
        },
      }),
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(await canvas.findByRole("textbox", { name: "Email" }), "bea@example.com");
    await userEvent.click(canvas.getByRole("button", { name: "Invite" }));
    await expect(
      await canvas.findByRole("textbox", { name: "Invitation link for bea@example.com" }),
    ).toBeVisible();
  },
};

/** Members, as a member: read-only. */
export const MembersReadOnly: Story = {
  args: {
    path: `/w/${HOUSEHOLD.id}/settings/members`,
    api: {
      ...signedIn([HOUSEHOLD]),
      [`GET /bff/api/v1/w/${HOUSEHOLD.id}/members`]: { body: { members: [ANA, SOFIA] } },
    },
  },
};

/** An invitation accepted. */
export const InvitationAccepted: Story = {
  args: {
    path: "/invitations/accept?token=demo",
    api: {
      ...signedIn([DUARTE]),
      "POST /bff/api/v1/workspaces/invitations/accept": { body: HOUSEHOLD },
    },
  },
};

/** An expired invitation. */
export const InvitationExpired: Story = {
  args: {
    path: "/invitations/accept?token=demo",
    api: {
      ...signedIn([DUARTE]),
      "POST /bff/api/v1/workspaces/invitations/accept": {
        status: 409,
        body: { status: 409, code: "invitation_expired", title: "Conflict" },
      },
    },
  },
};
