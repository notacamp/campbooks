import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import { addDays, isoDay } from "../../src/modules/tasks/model";
import { COUNTS, MARTA } from "../../test/demo/attention";
import {
  INBOX,
  PRIMARY,
  Q4,
  Q4_BODY,
  Q4_DETAIL,
  STREAM_COUNTS,
  W,
} from "../../test/demo/conversations";
import {
  ACCOUNTANT,
  BILL,
  INSPECTION,
  LEASE,
  PHOTOS,
  PROPOSAL,
  taskItem,
  TEAM,
  VENUE,
} from "../../test/demo/tasks";
import { ANA, DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";

// The Tasks place, Home's Do and Pay with tasks in them, and "Make it a
// task" in the reader, against fictional demo data. Dates are relative to the
// day the story runs, so the groups are always the same.

const PATH = `/w/${DUARTE.id}/tasks`;

/** Ana's tasks around today. */
function week() {
  const day = (n: number) => isoDay(addDays(new Date(), n));
  const bill = { ...BILL, due_on: day(-2), overdue: true };
  const lease = { ...LEASE, due_on: day(0) };
  const accountant = { ...ACCOUNTANT, plan_on: day(0) };
  const proposal = { ...PROPOSAL, due_on: day(5) };
  const inspection = {
    ...INSPECTION,
    later_until: addDays(new Date(), 10).toISOString(),
  };
  const venue = { ...VENUE, due_on: day(7) };
  return { bill, lease, accountant, proposal, inspection, venue };
}

/** The tasks API over a list the calls change, as the server would. */
function tasks(overrides: Record<string, MockRoute> = {}, start?: ReturnType<typeof week>) {
  const mine = start ?? week();
  let open = [mine.bill, mine.lease, mine.accountant, mine.proposal, PHOTOS, mine.inspection];
  let handedOff = [mine.venue];
  const routes: Record<string, MockRoute> = {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`GET ${W}/conversations`]: { body: { conversations: [MARTA], next_cursor: null } },
    [`GET ${W}/tasks`]: (_body, url) =>
      url.searchParams.get("whose") === "handed_off"
        ? { body: { tasks: handedOff } }
        : { body: { tasks: open } },
    [`POST ${W}/tasks`]: (body) => {
      const made = { ...PHOTOS, id: "0192f1a0-0000-7000-8000-0000000007ee", ...(body as object) };
      open = [...open, made];
      return { status: 201, body: made };
    },
    [`POST ${W}/tasks/${mine.bill.id}/done`]: () => {
      open = open.filter((task) => task.id !== mine.bill.id);
      return { body: { ...mine.bill, state: "done", revision: 2 } };
    },
    [`POST ${W}/tasks/${mine.bill.id}/reopen`]: () => {
      open = [mine.bill, ...open];
      return { body: { ...mine.bill, revision: 3 } };
    },
    [`POST ${W}/tasks/${mine.lease.id}/plan`]: {
      body: { ...mine.lease, plan: "later", revision: 2 },
    },
    [`POST ${W}/tasks/${mine.venue.id}/take_back`]: () => {
      const back = { ...mine.venue, handed_by: null, assignee: ANA.user_id, revision: 2 };
      handedOff = [];
      open = [...open, back];
      return { body: back };
    },
    [`GET ${W}/attention`]: { body: { items: [], counts: [] } },
    ...overrides,
  };
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
  title: "Screens/Tasks",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Overdue, Today, Coming up, No date yet, and Later folded. */
export const TheList: Story = {
  args: { path: PATH, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { level: 1, name: "Tasks" })).toBeVisible();
    await expect(await canvas.findByText("Send the signed lease to Marta")).toBeVisible();
    await expect(await canvas.findByText("Overdue · 2 days")).toBeVisible();
    await expect(canvas.queryByText("Book the car inspection")).toBeNull();
  },
};

/** A task open beside the list: when to do it, where it came from, hand-off. */
export const TaskOpen: Story = {
  args: { path: `${PATH}/${LEASE.id}`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Where it came from")).toBeVisible();
    await expect(await canvas.findByText("When will you do it?")).toBeVisible();
    await expect(canvas.getByRole("link", { name: "Open the email" })).toBeVisible();
  },
};

/** A bill says Paid, and that Campbooks never pays for you. */
export const BillOpen: Story = {
  args: { path: `${PATH}/${BILL.id}`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByText("Campbooks never pays for you. This only marks it paid."),
    ).toBeVisible();
  },
};

/** E marks the first task paid; the toast offers Undo. */
export const DoneWithUndo: Story = {
  args: { path: PATH, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("EDP electricity bill · €84.20");
    await userEvent.keyboard("e");
    await waitFor(() => expect(canvas.queryByText("EDP electricity bill · €84.20")).toBeNull());
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(/^Paid · EDP electricity bill/u)).toBeVisible();
  },
};

/** Type a title, pick Tomorrow, press Enter: it joins the list and the field is ready again. */
export const QuickAdd: Story = {
  args: { path: PATH, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const field = await canvas.findByRole("textbox", { name: "Add a task" });
    await userEvent.type(field, "Renew the parking permit");
    await userEvent.click(await canvas.findByRole("button", { name: "Tomorrow" }));
    await userEvent.type(field, "{Enter}");
    await expect(await canvas.findByText("Renew the parking permit")).toBeVisible();
    await expect(field).toHaveValue("");
  },
};

/** S offers Later: tonight, tomorrow, next week or a date. */
export const LaterOptions: Story = {
  args: { path: `${PATH}/${LEASE.id}`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("When will you do it?");
    await userEvent.click(canvas.getByRole("button", { name: /^Later/u }));
    await expect(await canvas.findByRole("button", { name: "Tomorrow" })).toBeVisible();
  },
};

/** A task handed to a teammate shows with their name, and can be taken back. */
export const HandedOff: Story = {
  args: { path: `${PATH}/${VENUE.id}`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // On the row and in the detail.
    await expect((await canvas.findAllByRole("button", { name: "Take it back" })).length).toBe(2);
  },
};

/** Nothing left: a fact, with the way to add a task. */
export const Empty: Story = {
  args: {
    path: PATH,
    api: tasks({
      [`GET ${W}/tasks`]: { body: { tasks: [] } },
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(await within(canvasElement).findByText("Nothing left to do")).toBeVisible();
  },
};

/** Home: tasks in Do and Pay beside the emails, with the same keys. */
export const OnHome: Story = {
  args: {
    path: `/w/${DUARTE.id}`,
    api: (() => {
      const mine = week();
      return tasks(
        {
          [`GET ${W}/attention`]: (_body, url) =>
            url.searchParams.get("state") === "open"
              ? {
                  body: {
                    items: [taskItem(mine.lease), taskItem(mine.bill)],
                    counts: COUNTS,
                  },
                }
              : { body: { items: [], counts: [] } },
          [`GET ${W}/tasks`]: { body: { tasks: [mine.lease, mine.bill] } },
        },
        mine,
      );
    })(),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Send the signed lease to Marta")).toBeVisible();
    await expect(canvas.getByText("EDP electricity bill · €84.20")).toBeVisible();
  },
};

/** In the reader, "Make it a task" opens a form with the subject as the title. */
export const MakeItATask: Story = {
  args: {
    path: `/w/${DUARTE.id}/inbox/primary/${Q4.id}`,
    api: tasks({
      [`GET ${W}/conversations`]: { body: { conversations: PRIMARY, next_cursor: null } },
      [`GET ${W}/conversations/${Q4.id}`]: { body: Q4_DETAIL },
      [`GET ${W}/conversations/${Q4.id}/messages/${Q4_DETAIL.messages[2]?.id ?? ""}/body`]: {
        body: Q4_BODY,
      },
      [`POST ${W}/conversations/${Q4.id}/read`]: { body: { ...Q4, unread_count: 0 } },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // N T makes a task from the email on screen.
    await canvas.findByRole("heading", { level: 2 });
    await userEvent.keyboard("nt");
    await expect(await canvas.findByRole("textbox", { name: "Title" })).toBeVisible();
  },
};

// Twins without a `play`, for the screen recordings: a story's play runs on
// load and would act before the recording starts.

/** The list, idle. */
export const ListIdle: Story = { args: { path: PATH, api: tasks() } };

/** Home with a task in Do and one in Pay, idle. */
export const HomeIdle: Story = { args: OnHome.args };

/** The reader on an email, idle. */
export const ReaderIdle: Story = { args: MakeItATask.args };
