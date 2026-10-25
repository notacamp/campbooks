import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import type { TaskView } from "~/modules/tasks";

import { createAppRouter } from "../../src/app/router";
import { addDays, isoDay } from "../../src/modules/tasks/model";
import { COUNTS, LUZ, MARTA } from "../../test/demo/attention";
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
  ASK_BILL,
  ASK_LEASE,
  BILL,
  INSPECTION,
  LEASE,
  PHOTOS,
  PROPOSAL,
  taskItem,
  TEAM,
  VENUE,
} from "../../test/demo/tasks";
import { CALENDARS } from "../../test/demo/schedule";
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

/** Ana's free time: tomorrow morning and the afternoon after, on her clock. */
function freeTime() {
  const at = (days: number, hours: number) => {
    const day = addDays(new Date(), days);
    day.setHours(hours, 0, 0, 0);
    return day.toISOString();
  };
  return [
    { starts: at(1, 9), ends: at(1, 12) },
    { starts: at(2, 14), ends: at(2, 15) },
  ];
}

/** The task as `tasks.hold` answers it: held, and planned for that day. */
function held<T extends TaskView>(
  task: T,
  body: { starts: string; ends: string; calendar_id?: string },
) {
  return {
    ...task,
    plan: "date" as const,
    plan_on: isoDay(new Date(body.starts)),
    hold: {
      event_id: "0192f1a0-0000-7000-8000-0000000004e1",
      calendar_id: body.calendar_id ?? CALENDARS[0]?.id ?? "",
      starts: body.starts,
      ends: body.ends,
    },
    revision: task.revision + 1,
  };
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
    [`GET ${W}/tasks`]: (_body, url) => {
      if (url.searchParams.get("whose") === "handed_off") {
        return { body: { tasks: handedOff } };
      }
      // Home's batch read of the tasks its items point at.
      const ids = url.searchParams.get("ids");
      return ids === null
        ? { body: { tasks: open } }
        : { body: { tasks: open.filter((task) => ids.split(",").includes(task.id)) } };
    },
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
    // Hold time: Ana's free time and calendars (the schedule module's), and the hold.
    [`GET ${W}/schedule/free-slots`]: () => ({
      body: { slots: freeTime(), calendars: [], time_zone: "Europe/Lisbon" },
    }),
    [`GET ${W}/schedule/calendars`]: { body: { calendars: CALENDARS } },
    [`POST ${W}/tasks/${mine.lease.id}/hold`]: (body) => {
      const after = held(mine.lease, body as { starts: string; ends: string });
      open = open.map((task) => (task.id === after.id ? after : task));
      return { body: after };
    },
    [`POST ${W}/tasks/${mine.lease.id}/release`]: () => {
      const after = { ...mine.lease, plan: "none" as const, hold: null, revision: 3 };
      open = open.map((task) => (task.id === after.id ? after : task));
      return { body: after };
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

/**
 * The new-task form in the right pane (New task, or N T): a title, Tomorrow,
 * Enter. It joins the list and the form is ready for the next one.
 */
export const NewTask: Story = {
  args: { path: `${PATH}?new=1`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const field = await canvas.findByRole("textbox", { name: "Task title" });
    await userEvent.type(field, "Renew the parking permit");
    await userEvent.click(await canvas.findByRole("button", { name: "Tomorrow" }));
    await userEvent.type(field, "{Enter}");
    await expect(await canvas.findByText("Renew the parking permit")).toBeVisible();
    await expect(field).toHaveValue("");
  },
};

/** Pick a date: the month grid in place, under the chips. */
export const NewTaskPickADate: Story = {
  args: { path: `${PATH}?new=1`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Pick a date" }));
    await expect(await canvas.findByRole("grid")).toBeVisible();
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

/**
 * H offers Hold time: three free slots, Length and Calendar (her primary Google
 * calendar first); picking one holds it, with an Undo.
 */
export const HoldTime: Story = {
  args: { path: `${PATH}/${LEASE.id}`, api: tasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("When will you do it?");
    await userEvent.keyboard("h");
    const slots = await canvas.findByRole("group", { name: "Free time" });
    const choices = await within(slots).findAllByRole("button");
    await expect(choices).toHaveLength(3);
    await expect(canvas.getByRole("button", { name: "30 min", pressed: true })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Personal", pressed: true })).toBeVisible();
    await userEvent.click(choices[0] ?? slots);
    await expect((await canvas.findAllByText(/^Held /u)).length).toBeGreaterThan(0);
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(/· Send the signed lease to Marta$/u)).toBeVisible();
  },
};

/** A held task: the chip in Do's colour on its row and in its detail, and Release. */
export const Held: Story = {
  args: (() => {
    const mine = week();
    const [first] = freeTime();
    const lease = held(mine.lease, first ?? { starts: "", ends: "" });
    return { path: `${PATH}/${LEASE.id}`, api: tasks({}, { ...mine, lease }) };
  })(),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect((await canvas.findAllByText(/^Held /u)).length).toBe(2);
    await expect(canvas.getByRole("button", { name: "Release" })).toBeVisible();
  },
};

/** Home with the lease in Do and the bill in Pay, over the tasks API the calls change. */
function homeWithTasks() {
  const mine = week();
  return tasks(
    {
      [`GET ${W}/attention`]: (_body, url) =>
        url.searchParams.get("state") === "open"
          ? { body: { items: [taskItem(mine.lease), taskItem(mine.bill)], counts: COUNTS } }
          : { body: { items: [], counts: [] } },
    },
    mine,
  );
}

/** Home: H on a Do task opens Hold time in a dialog; the row then shows Held. */
export const HoldFromHome: Story = {
  args: { path: `/w/${DUARTE.id}`, api: homeWithTasks() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Send the signed lease to Marta");
    await userEvent.keyboard("j");
    await userEvent.keyboard("h");
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Hold time" });
    const slots = await within(dialog).findByRole("group", { name: "Free time" });
    const [first] = await within(slots).findAllByRole("button");
    await userEvent.click(first ?? slots);
    await expect((await canvas.findAllByText(/^Held /u)).length).toBeGreaterThan(0);
  },
};

// Twins without a `play`, for the screen recordings: a story's play runs on
// load and would act before the recording starts.

/** A task open beside the list, ready for H, idle. */
export const HoldTimeIdle: Story = { args: HoldTime.args };

/** A held task, idle. */
export const HeldIdle: Story = { args: Held.args };

/** Home before H on the lease, idle. */
export const HoldFromHomeIdle: Story = { args: HoldFromHome.args };

/** The list, idle. */
export const ListIdle: Story = { args: { path: PATH, api: tasks() } };

/** Home with a task in Do and one in Pay, idle. */
export const HomeIdle: Story = { args: OnHome.args };

/** The reader on an email, idle. */
export const ReaderIdle: Story = { args: MakeItATask.args };

/** What Scout found in two emails, as Ana's tasks around today. */
function found() {
  const day = (n: number) => isoDay(addDays(new Date(), n));
  const lease = { ...ASK_LEASE, due_on: day(3) };
  const bill = { ...ASK_BILL, due_on: day(-1) };
  return { lease, bill };
}

/** The API for tasks Scout made from emails. */
function foundApi() {
  const { lease, bill } = found();
  return tasks({
    [`GET ${W}/conversations`]: { body: { conversations: [MARTA, LUZ], next_cursor: null } },
    [`GET ${W}/tasks`]: { body: { tasks: [bill, lease] } },
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "open"
        ? { body: { items: [taskItem(bill), taskItem(lease)], counts: COUNTS } }
        : { body: { items: [], counts: [] } },
  });
}

/** A task Scout made from an email says so, above where it came from. */
export const FoundByScout: Story = {
  args: { path: `${PATH}/${ASK_BILL.id}`, api: foundApi() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Scout found this in the email")).toBeVisible();
    await expect(canvas.getByRole("link", { name: "Open the email" })).toBeVisible();
  },
};

/** Home with the two tasks Scout made from emails (Pay and Do), idle. */
export const FoundOnHomeIdle: Story = {
  args: { path: `/w/${DUARTE.id}`, api: foundApi() },
};

/** The detail of the invoice's task, idle. */
export const FoundByScoutIdle: Story = {
  args: { path: `${PATH}/${ASK_BILL.id}`, api: foundApi() },
};
