import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  AGENT_CONSENT,
  INSTANCE_APP_CONSENT,
  NATIVE_CONSENT,
  REQUEST,
  TEAM_APP_CONSENT,
} from "../../test/demo/identity";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// The consent screen (`/consent`), in the real app against in-memory demo
// data: every state, tested with axe in light and dark. No story leaves the
// page: answering is mocked to fail, or not sent.

const problem = (status: number, code: string): MockRoute => ({
  status,
  body: { type: "about:blank", title: "Problem", status, code },
});

const PAGE = `/consent?request=${REQUEST}`;

const asking = (body: unknown, answer?: MockRoute): Record<string, MockRoute> => ({
  "GET /bff/session": { body: { authenticated: false } },
  [`GET /auth/consent/${REQUEST}`]: { body },
  ...(answer === undefined ? {} : { [`POST /auth/consent/${REQUEST}`]: answer }),
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
  title: "Screens/Consent",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** An MCP agent on this device: unverified, warned about, one workspace to pick. */
export const AgentOnThisDevice: Story = {
  args: { path: PAGE, api: asking(AGENT_CONSENT) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Only if you just started this")).toBeVisible();
    const allow = canvas.getByRole("button", { name: "Allow" });
    await expect(allow).toBeDisabled();
    await expect(canvas.getByRole("button", { name: /Deny/u })).toHaveFocus();
    await userEvent.click(canvas.getByRole("radio", { name: "Duarte & Pessoa" }));
    await waitFor(
      async () => {
        await expect(allow).toBeEnabled();
      },
      { timeout: 3000 },
    );
  },
};

/** Campbooks' own desktop app: verified, all workspaces, asked every time. */
export const DesktopApp: Story = {
  args: { path: PAGE, api: asking(NATIVE_CONSENT) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("It will work in all your workspaces, like Campbooks on the web."),
    ).toBeVisible();
    await expect(canvas.getByText(/asked each time/u)).toBeVisible();
  },
};

/** A workspace's own sign-in app on an https callback: remembered once allowed. */
export const WorkspaceApp: Story = {
  args: { path: PAGE, api: asking(TEAM_APP_CONSENT) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("radio", { name: "Duarte & Pessoa" })).not.toBeChecked();
    await expect(canvas.getByText("Duarte & Pessoa’s app")).toBeVisible();
    await expect(canvas.getByText("Next, you’ll go to dashboard.example.com.")).toBeVisible();
  },
};

/** An app this server's administrator added (Applicamp): vouched for, one workspace to pick. */
export const InstanceApp: Story = {
  args: { path: PAGE, api: asking(INSTANCE_APP_CONSENT) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Added by this server")).toBeVisible();
    await expect(
      canvas.getByText("Added to this Campbooks by the people who run it."),
    ).toBeVisible();
    await expect(canvas.queryByText("Anyone can register an app with any name")).toBeNull();
    await expect(canvas.getByText("See your calendars and events")).toBeVisible();
    await expect(canvas.getByRole("radio", { name: "Household" })).not.toBeChecked();
  },
};

/** A metadata-document client on the web: unverified, described by its host. */
export const DescribedByItsHost: Story = {
  args: {
    path: PAGE,
    api: asking({
      ...AGENT_CONSENT,
      client: {
        ...AGENT_CONSENT.client,
        name: "Research Assistant",
        kind: "metadata_document",
        document_host: "agent.example.com",
        redirect_host: "agent.example.com",
        device: false,
      },
      remembered: true,
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("Described by agent.example.com. Nobody has checked who made it."),
    ).toBeVisible();
    await expect(canvas.getByText("Anyone can register an app with any name")).toBeVisible();
  },
};

/** Not in any workspace yet: nothing to allow. */
export const NoWorkspace: Story = {
  args: { path: PAGE, api: asking({ ...AGENT_CONSENT, workspaces: [] }) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Join a workspace first")).toBeVisible();
  },
};

/** The sign-in is too old to let an app in. */
export const SignInAgainFirst: Story = {
  args: { path: PAGE, api: asking(NATIVE_CONSENT, problem(403, "step_up_required")) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const allow = await canvas.findByRole("button", { name: "Allow" });
    await waitFor(
      async () => {
        await expect(allow).toBeEnabled();
      },
      { timeout: 3000 },
    );
    await userEvent.click(allow);
    await expect(await canvas.findByText("Sign in again first")).toBeVisible();
  },
};

/** Expired, answered, or someone else's. */
export const NoLongerWaiting: Story = {
  args: {
    path: PAGE,
    api: {
      "GET /bff/session": { body: { authenticated: false } },
      [`GET /auth/consent/${REQUEST}`]: problem(404, "not_found"),
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("This request is no longer waiting")).toBeVisible();
  },
};
