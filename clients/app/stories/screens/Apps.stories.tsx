import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { CONNECTED_APPS, DEMO_SECRET, WORKSPACE_CLIENTS } from "../../test/demo/identity";
import { DUARTE, HOUSEHOLD, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// Settings → Apps and integrations, in the real app against in-memory demo
// data: every state, tested with axe in light and dark.

const problem = (status: number, code: string): MockRoute => ({
  status,
  body: { type: "about:blank", title: "Problem", status, code },
});

const pageOf = (workspace: string) => `/w/${workspace}/settings/apps`;

function signedIn({
  apps = CONNECTED_APPS,
  clients = WORKSPACE_CLIENTS,
  register,
  rotate,
}: {
  apps?: readonly unknown[];
  clients?: readonly unknown[];
  register?: MockRoute;
  rotate?: MockRoute;
}): Record<string, MockRoute> {
  const routes: Record<string, MockRoute> = {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE, HOUSEHOLD] } },
    "GET /bff/api/v1/identity/consents": { body: { items: apps } },
    [`GET /bff/api/v1/w/${DUARTE.id}/identity/clients`]: { body: { items: clients } },
    [`DELETE /bff/api/v1/identity/consents/${CONNECTED_APPS[0]?.id ?? ""}`]: { status: 204 },
  };
  if (register !== undefined) {
    routes[`POST /bff/api/v1/w/${DUARTE.id}/identity/clients`] = register;
  }
  if (rotate !== undefined) {
    for (const client of WORKSPACE_CLIENTS) {
      routes[`POST /bff/api/v1/w/${DUARTE.id}/identity/clients/${client.client_id}/secret`] =
        rotate;
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
  title: "Screens/Apps and integrations",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** An agent and the desktop app let in; the studio's nightly export. */
export const Overview: Story = {
  args: { path: pageOf(DUARTE.id), api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Desk Agent")).toBeVisible();
    await expect(await canvas.findByText("Nightly export")).toBeVisible();
    await expect(canvas.getByRole("button", { name: /New app/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "Shift+N",
    );
  },
};

/** Nothing let in, no apps yet. */
export const Empty: Story = {
  args: { path: pageOf(DUARTE.id), api: signedIn({ apps: [], clients: [] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("No app can act as you.")).toBeVisible();
    await expect(await canvas.findByText("No apps yet.")).toBeVisible();
  },
};

/** Revoke, confirmed in place. */
export const ConfirmRevoke: Story = {
  args: { path: pageOf(DUARTE.id), api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Revoke Desk Agent" }));
    await expect(canvas.getByText(/stops working at once/u)).toBeVisible();
  },
};

/** The new-app form, a sign-in app with its return address. */
export const NewSignInApp: Story = {
  args: { path: pageOf(DUARTE.id), api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /New app/u }));
    await userEvent.type(canvas.getByRole("textbox", { name: "Name" }), "Studio dashboard");
    await userEvent.click(canvas.getByRole("radio", { name: "Sign-in app" }));
    await expect(canvas.getByRole("textbox", { name: "Return address" })).toBeVisible();
  },
};

/** A server-to-server app created: its secret, this once, with Copy. */
export const SecretShownOnce: Story = {
  args: {
    path: pageOf(DUARTE.id),
    api: signedIn({
      register: {
        status: 201,
        body: {
          client: { ...WORKSPACE_CLIENTS[0], client_id: "cb_weekly", name: "Weekly report" },
          client_secret: DEMO_SECRET,
        },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: /New app/u }));
    await userEvent.type(canvas.getByRole("textbox", { name: "Name" }), "Weekly report");
    await userEvent.click(canvas.getByRole("button", { name: "Create app" }));
    await expect(await canvas.findByText("Copy the secret now")).toBeVisible();
    await expect(canvas.getByDisplayValue(DEMO_SECRET)).toBeVisible();
  },
};

/** Rotating needs a recent sign-in. */
export const SignInAgainFirst: Story = {
  args: { path: pageOf(DUARTE.id), api: signedIn({ rotate: problem(403, "step_up_required") }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", { name: "Rotate the secret of Nightly export" }),
    );
    await userEvent.click(canvas.getByRole("button", { name: "Rotate secret" }));
    await expect(await canvas.findByText("Sign in again first")).toBeVisible();
  },
};

/** A plain member: their connected apps, but not the workspace's. */
export const Member: Story = {
  args: { path: pageOf(HOUSEHOLD.id), api: signedIn({}) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("Only owners and admins can manage this workspace’s apps."),
    ).toBeVisible();
  },
};
