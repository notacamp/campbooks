import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  NOT_FOUND_PROBLEM,
  OVERVIEW,
  OVERVIEW_WITHOUT_AI,
  REAUTH_PROBLEM,
} from "../../test/demo/admin";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, type MockRoute } from "./mock-fetch";

// The admin section (ADR 0015) in the real app, against demo data: every
// state of the System panel and of the way in, tested with axe in light and
// dark.

const OVERVIEW_ROUTE = "GET /bff/admin/v1/system/overview";

const signedIn = (overview: MockRoute): Record<string, MockRoute> => ({
  "GET /bff/session": { body: SESSION },
  "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
  [OVERVIEW_ROUTE]: overview,
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
  title: "Screens/Admin",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args }) => mockFetch(args.api),
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Admin → System for a super admin: build, database, configuration, services, AI. */
export const System: Story = {
  args: { path: "/admin/system", api: signedIn({ body: OVERVIEW }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { level: 1, name: "System" })).toBeVisible();
    await expect(await canvas.findByText("Campbooks’ own")).toBeVisible();
  },
};

/** No AI role set up: every role says so, nothing is an error. */
export const SystemWithoutAi: Story = {
  args: { path: "/admin/system", api: signedIn({ body: OVERVIEW_WITHOUT_AI }) },
};

/** The database stopped answering: the overview still loads and says so. */
export const SystemDatabaseDown: Story = {
  args: {
    path: "/admin/system",
    api: signedIn({
      body: {
        ...OVERVIEW,
        database: {
          reachable: false,
          latency_ms: null,
          migrations_applied: null,
          latest_migration: null,
          latest_migration_description: null,
        },
      },
    }),
  },
};

/** The last sign-in is more than 12 hours old: sign in again, back to this page. */
export const SignInAgain: Story = {
  args: { path: "/admin/system", api: signedIn({ status: 401, body: REAUTH_PROBLEM }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("link", { name: "Sign in again" })).toHaveAttribute(
      "href",
      "/bff/login?return_to=%2Fadmin%2Fsystem&max_age=0",
    );
  },
};

/** Not an administrator: /admin is a page that does not exist. */
export const NotAnAdministrator: Story = {
  args: { path: "/admin", api: signedIn({ status: 404, body: NOT_FOUND_PROBLEM }) },
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByRole("heading", { name: "Page not found" }),
    ).toBeVisible();
  },
};

/** The server can't be reached while opening administration. */
export const Unreachable: Story = {
  args: {
    path: "/admin",
    api: signedIn({ status: 503, body: { status: 503, code: "database_unavailable" } }),
  },
};

/** An administrator's account menu: Administration is one of its entries. */
export const AccountMenuEntry: Story = {
  args: { path: `/w/${DUARTE.id}`, api: signedIn({ body: OVERVIEW }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { name: "Nothing needs you yet" });
    await userEvent.click(canvas.getByRole("button", { name: /Ana Duarte/u }));
    await expect(await canvas.findByRole("link", { name: "Administration" })).toBeVisible();
  },
};
