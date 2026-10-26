import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { UndoProvider } from "~/lib/ui";

import { createAppRouter } from "../../src/app/router";
import {
  ANA,
  ANA_DETAIL,
  ANA_ITEM,
  CLEARED,
  COPIED,
  COPIED_DETAIL,
  COPIED_ITEM,
  COPIED_WAITING,
  COUNTS,
  INES,
  ITEMS,
  MARTA,
  MARTA_DETAIL,
  NEEDS,
  PHISH,
  PHISH_DETAIL,
  RUI,
  RUI_DETAIL,
  SOFIA,
  TIAGO,
  TIAGO_DETAIL,
  WAITING,
} from "../../test/demo/attention";
import { INBOX, NOW, STREAM_COUNTS, W } from "../../test/demo/conversations";
import {
  HOME_CALENDARS,
  HOME_COUNTS,
  HOME_COUNTS_INVITED,
  HOME_DUE,
  HOME_EVENTS,
  HOME_INVITATION_EVENTS,
  HOME_ITEMS,
  HOME_ITEMS_INVITED,
  HOME_NEEDS,
  HOME_SETTINGS,
  HOME_TASKS,
  HOME_TASKS_ELSEWHERE,
} from "../../test/demo/home";
import { DUARTE, SESSION } from "../../test/demo/workspaces";

import { mockFetch, saveThemeFromGlobals, type MockRoute } from "./mock-fetch";
import { pinClock, scheduleApi } from "./schedule-mock";

// Home with mail connected: what needs you, against fictional demo data.
// Every state, tested with axe in light and dark.

const PATH = `/w/${DUARTE.id}`;

function home(overrides: Record<string, MockRoute> = {}): Record<string, MockRoute> {
  return {
    "GET /bff/session": { body: SESSION },
    "GET /bff/api/v1/workspaces": { body: { workspaces: [DUARTE] } },
    [`GET ${W}/inboxes`]: { body: { inboxes: [INBOX] } },
    [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } },
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: ITEMS, counts: COUNTS } },
    [`GET ${W}/conversations`]: { body: { conversations: [...NEEDS, SOFIA], next_cursor: null } },
    [`GET ${W}/conversations/${MARTA.id}`]: { body: MARTA_DETAIL },
    [`GET ${W}/conversations/${PHISH.id}`]: { body: PHISH_DETAIL },
    [`POST ${W}/conversations/${PHISH.id}/need/resolve`]: { body: { ...PHISH, need: null } },
    [`POST ${W}/conversations/${PHISH.id}/need/reopen`]: { body: PHISH },
    ...overrides,
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
}

const meta = {
  title: "Screens/Home",
  parameters: { layout: "fullscreen" },
  beforeEach: ({ args, globals }) => {
    saveThemeFromGlobals(globals);
    return mockFetch(args.api);
  },
  render: ({ path }) => <AppAt path={path} />,
} satisfies Meta<ScreenArgs>;

export default meta;
type Story = StoryObj<typeof meta>;

/** By band, Now first; each row says what it is and why first; no percentage anywhere. */
export const WhatNeedsYou: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "6 things need you" }),
    ).toBeVisible();
    await expect(canvas.getByText("Marta Ferreira is waiting for your reply")).toBeVisible();
    await expect(canvas.getByText("Replied · Tile samples")).toBeVisible();
  },
};

/** Enter opens the warning: its tells first, then the sender's words, then how it was decided. */
export const WarningOpen: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("This may be phishing. Don’t use its links.");
    await userEvent.keyboard("{Enter}");
    await expect(
      await canvas.findByText(
        "The sender’s domain looks like a better-known one (at3lier-sol.example).",
      ),
    ).toBeVisible();
  },
};

/** X marks the warning safe; the toast offers Undo (Z). */
export const SafeWithUndo: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("This may be phishing. Don’t use its links.");
    await userEvent.keyboard("x");
    await waitFor(() =>
      expect(canvas.queryByText("This may be phishing. Don’t use its links.")).toBeNull(),
    );
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText(/Marked safe/u)).toBeVisible();
  },
};

/** S opens Snooze: later today, tomorrow, Monday or next week, in a dialog. */
export const SnoozeDialog: Story = {
  args: { path: PATH, api: home() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Marta Ferreira is waiting for your reply");
    // Past Luz's bill (Now) to Marta (Today).
    await userEvent.keyboard("jj");
    await userEvent.keyboard("s");
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Snooze Marta Ferreira until…" });
    // It rises in over a moment.
    await waitFor(async () => {
      await expect(dialog).toBeVisible();
    });
  },
};

/** One Server-Sent Events frame, as the API sends it. */
function frame(event: string, data: unknown): string {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

const REPLY_ROUTES: Record<string, MockRoute> = {
  [`POST ${W}/conversations/${MARTA.id}/reply/suggestions`]: {
    body: {
      message_id: MARTA_DETAIL.messages.at(-1)?.id,
      intents: [
        "Yes, Thursday works",
        "Ask to move it to the afternoon",
        "Say you'll confirm by tomorrow",
      ],
    },
  },
  // The draft streams on the web: Scout's thinking, then the words.
  [`POST ${W}/conversations/${MARTA.id}/reply/draft/stream`]: {
    stream: [
      frame("thinking", { delta: "Marta asks whether Thursday works for the walkthrough." }),
      frame("text", { delta: "Hi Marta,\n\nThursday works for me. " }),
      frame("text", { delta: "See you at the walkthrough.\n\nBest,\nSam Rivera" }),
      frame("done", {
        message_id: MARTA_DETAIL.messages.at(-1)?.id,
        text: "Hi Marta,\n\nThursday works for me. See you at the walkthrough.\n\nBest,\nSam Rivera",
      }),
    ],
  },
};

/** R opens Reply: Scout's likely replies, each with its key, and Write my own. */
export const ReplyDialog: Story = {
  args: { path: PATH, api: home(REPLY_ROUTES) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Marta Ferreira is waiting for your reply");
    await userEvent.keyboard("jj");
    await userEvent.keyboard("r");
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Reply to Marta" });
    // The dialog fades in: wait until it is shown.
    await waitFor(async () => {
      await expect(dialog).toBeVisible();
    });
    const first = await body.findByRole("button", { name: /Yes, Thursday works/u });
    await waitFor(async () => {
      await expect(first).toHaveFocus();
    });
  },
};

/** A pick asks Scout for the draft: editable, with Shorter, Change and Send. */
export const ReplyDraft: Story = {
  args: { path: PATH, api: home(REPLY_ROUTES) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Marta Ferreira is waiting for your reply");
    await userEvent.keyboard("jj");
    await userEvent.keyboard("r");
    const body = within(canvasElement.ownerDocument.body);
    await body.findByRole("button", { name: /Yes, Thursday works/u });
    await userEvent.keyboard("1");
    const text = await body.findByRole("textbox", { name: "Your reply" });
    await waitFor(async () => {
      await expect(text).toHaveFocus();
    });
  },
};

/** No model may read mail on this server: only Write my own, saying why. */
export const ReplyUnavailable: Story = {
  args: {
    path: PATH,
    api: home({
      [`POST ${W}/conversations/${MARTA.id}/reply/suggestions`]: {
        status: 409,
        body: { status: 409, code: "reply_unavailable", title: "Unavailable" },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Marta Ferreira is waiting for your reply");
    await userEvent.keyboard("jj");
    await userEvent.keyboard("r");
    const body = within(canvasElement.ownerDocument.body);
    // The dialog fades in: wait until its line is shown.
    await waitFor(async () => {
      await expect(
        body.getByText("Scout can't write replies on this server, so you write this one."),
      ).toBeVisible();
    });
  },
};

/** Nothing needs you: a fact, not a promise, with Done today still there. */
export const NothingNeedsYou: Story = {
  args: {
    path: PATH,
    api: home({
      [`GET ${W}/attention`]: (_body, url) =>
        url.searchParams.get("state") === "cleared"
          ? { body: { items: CLEARED, counts: [] } }
          : { body: { items: [], counts: [] } },
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByRole("heading", { level: 1, name: "Nothing needs you" }),
    ).toBeVisible();
  },
};

/** The server could not be reached. */
export const LoadError: Story = {
  args: {
    path: PATH,
    api: home({
      [`GET ${W}/attention`]: {
        status: 503,
        body: { status: 503, code: "unavailable", title: "Unavailable" },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByText("Could not load what needs you"),
    ).toBeVisible();
  },
};

// A busy morning: forty things need you, more than fit in the window.
const MANY_NEEDS = Array.from({ length: 40 }, (_, index) => {
  const summary = NEEDS[index % NEEDS.length] ?? MARTA;
  return index < NEEDS.length ? summary : { ...summary, id: `${summary.id}-${String(index)}` };
});
const MANY_ITEMS = MANY_NEEDS.map((summary, index) => {
  const base = ITEMS[index % ITEMS.length] ?? ITEMS[0];
  return index < ITEMS.length || base === undefined
    ? base
    : {
        ...base,
        id: `${base.id}-${String(index)}`,
        record: { ...base.record, id: summary.id },
      };
}).filter((item) => item !== undefined);

/** A viewport the story test runs at (the addon reads `globals.viewport`). */
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

const busy: Pick<Story, "args" | "play"> = {
  args: {
    path: PATH,
    api: home({
      [`GET ${W}/attention`]: (_body, url) =>
        url.searchParams.get("state") === "cleared"
          ? { body: { items: CLEARED, counts: [] } }
          : { body: { items: MANY_ITEMS, counts: COUNTS } },
      [`GET ${W}/conversations`]: {
        body: { conversations: [...MANY_NEEDS, SOFIA], next_cursor: null },
      },
    }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 1, name: "40 things need you" });
    const page = canvas.getByRole("main");
    const [sidebar] = canvas.getAllByRole("navigation");
    // Forty needs are taller than the window, so Home scrolls inside its pane…
    await waitFor(() => expect(page.scrollHeight).toBeGreaterThan(page.clientHeight));
    page.scrollTop = page.scrollHeight;
    await waitFor(() => expect(page.scrollTop).toBeGreaterThan(0));
    // …while the page itself stays the window's height and the sidebar stays put.
    await expect(document.documentElement.scrollHeight).toBeLessThanOrEqual(globalThis.innerHeight);
    await expect(sidebar?.getBoundingClientRect().bottom).toBe(globalThis.innerHeight);
  },
};

/** A busy Home in a narrow desktop window: Home scrolls, the page doesn't. */
export const BusyNarrowDesktop: Story = { ...busy, ...at("narrowDesktop", 1100, 800) };

/** A busy Home in a wide desktop window: Home scrolls, the page doesn't. */
export const BusyDesktop: Story = { ...busy, ...at("desktop", 1440, 900) };

// Waits and nudges (campbooks-cloud#293): Ana Costa hasn't written since
// João asked on 7 October, and Scout waits on Rui Matos and Inês Cardoso.
function waits(
  open = [...ITEMS.slice(0, 2), ANA_ITEM, ...ITEMS.slice(2)],
): Record<string, MockRoute> {
  return home({
    [`GET ${W}/attention`]: (_body, url) => {
      const state = url.searchParams.get("state");
      if (state === "cleared") {
        return { body: { items: CLEARED, counts: [] } };
      }
      if (state === "waiting") {
        return { body: { items: WAITING, counts: [{ reason: "waiting", count: WAITING.length }] } };
      }
      return { body: { items: open, counts: COUNTS } };
    },
    [`GET ${W}/conversations`]: {
      body: { conversations: [...NEEDS, ANA, RUI, INES, SOFIA], next_cursor: null },
    },
    [`GET ${W}/conversations/${ANA.id}`]: { body: ANA_DETAIL },
    [`GET ${W}/conversations/${RUI.id}`]: { body: RUI_DETAIL },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`POST ${W}/conversations/${ANA.id}/nudge/draft`]: {
      body: {
        message_id: ANA.need?.message_id,
        text: "Hi Ana,\n\nJust following up on João’s note below: could you send me the revised floor plan when you have a moment? I’d like to go through it before Thursday’s visit.\n\nThanks,\nSam Rivera",
      },
    },
  });
}

/** In Today: Ana hasn't written. Waiting on others sits above Done today. */
export const NudgeAndWaiting: Story = {
  args: { path: PATH, api: waits() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Ana Costa hasn’t written yet")).toBeVisible();
    await expect(
      await canvas.findByRole("heading", { level: 2, name: "Waiting on others" }),
    ).toBeVisible();
    // When Scout checks Rui again, as a weekday or a date (the story's clock is today's).
    await expect(canvas.getAllByText(/^Checks /u)[0]).toBeVisible();
  },
};

const nudgeOpen: Pick<Story, "args" | "play"> = {
  args: { path: PATH, api: waits() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByText("Ana Costa hasn’t written yet"));
    await userEvent.keyboard("1");
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Nudge Ana" });
    await waitFor(async () => {
      await expect(dialog).toBeVisible();
    });
    await expect(await body.findByRole("textbox", { name: "Your nudge" })).toBeVisible();
    await expect(body.getByText("João Matos <joao@obras-matos.example>")).toBeVisible();
  },
};

/** 1 on Ana's item: Scout's nudge, to Ana, copying João, editable before Send. */
export const NudgeDialog: Story = nudgeOpen;

/** The nudge dialog on a phone: a bottom sheet, nothing scrolls sideways. */
export const NudgeDialogPhone: Story = { ...nudgeOpen, ...at("phone", 375, 812) };

/** On a phone, Waiting on others is a fold after the list, with its count. */
export const WaitingPhone: Story = {
  args: { path: PATH, api: waits() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByText("Waiting on others · 2")).toBeVisible();
    await expect(canvasElement.ownerDocument.documentElement.scrollWidth).toBeLessThanOrEqual(
      globalThis.innerWidth,
    );
  },
};

/** Nothing needs you, but Scout waits on two people: said under the hero. */
export const NothingNeedsYouWaiting: Story = {
  args: { path: PATH, api: waits([]) },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "Nothing needs you" }),
    ).toBeVisible();
    await expect(
      await canvas.findByText(/Scout is also waiting on 2 people for you\./u),
    ).toBeVisible();
  },
};

// A whole morning across the four bands (campbooks-cloud#358): a scam
// warning and a late bill, a nudge, the lease due today, Marta's
// walkthrough tomorrow and Tiago waiting four days, a bill on Thursday and
// two people waiting for an answer, a bill on the 30th and a newsletter a
// rule keeps; three more tasks in Tasks.
function morning(): Record<string, MockRoute> {
  return home({
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [], tasks_elsewhere: 0 } }
        : {
            body: {
              items: HOME_ITEMS,
              counts: HOME_COUNTS,
              tasks_elsewhere: HOME_TASKS_ELSEWHERE,
            },
          },
    [`GET ${W}/conversations`]: {
      body: { conversations: [...HOME_NEEDS, SOFIA], next_cursor: null },
    },
    [`GET ${W}/tasks`]: { body: { tasks: HOME_TASKS } },
    [`GET ${W}/conversations/${TIAGO.id}`]: { body: TIAGO_DETAIL },
    [`GET ${W}/conversations/${ANA.id}`]: { body: ANA_DETAIL },
  });
}

/** Now, Today, This week, Whenever; "3 more in Tasks" ends the list. */
export const Bands: Story = {
  args: { path: PATH, api: morning() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "10 things need you" }),
    ).toBeVisible();
    const groups = canvas.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    await expect(groups.slice(0, 4)).toEqual(["Now 2", "Today 4", "This week 3", "Whenever 2"]);
    await expect(canvas.getByText("Tiago has waited 4 days")).toBeVisible();
    await expect(canvas.getByText("3 more in Tasks")).toBeVisible();
  },
};

/** The same morning on a phone: the eyebrow wraps, nothing scrolls sideways. */
export const BandsPhone: Story = {
  args: { path: PATH, api: morning() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Tiago has waited 4 days");
    await expect(canvasElement.ownerDocument.documentElement.scrollWidth).toBeLessThanOrEqual(
      globalThis.innerWidth,
    );
  },
};

/** Opened, Tiago's reply says why it is in Today. */
export const WhyItsHere: Story = {
  args: { path: PATH, api: morning() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByText("Tiago Nunes is waiting for your reply"));
    const why = await canvas.findByRole("region", { name: "Why it’s here" });
    await expect(why).toHaveTextContent("In Today: Tiago has waited 4 days.");
  },
};

// Your calendar on Home (campbooks-cloud#359), on the demo's Tuesday 13
// October at 15:00 UTC, 11:00 on Sam's New York calendar: a call in 20
// minutes with a web address, lunch, time held for the lease, a key
// handover read from Marta's email; Coming up with a delivery and a train
// only Sam sees, a charge, a task due Friday and a renewal.
function yourDay(): Record<string, MockRoute> {
  const schedule = scheduleApi(HOME_EVENTS, { calendars: HOME_CALENDARS });
  return {
    ...morning(),
    ...schedule.routes,
    [`GET ${W}/schedule/settings`]: { body: HOME_SETTINGS },
    [`GET ${W}/tasks`]: { body: { tasks: [...HOME_TASKS, HOME_DUE] } },
  };
}

const onDemoDay: Pick<Story, "beforeEach"> = {
  beforeEach: () => pinClock(NOW.toISOString()),
};

/** Next up above Now, today's events at the end of Today, Coming up in the aside. */
export const YourDay: Story = {
  ...onDemoDay,
  args: { path: PATH, api: yourDay() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(await canvas.findByRole("heading", { level: 2, name: "Next up" })).toBeVisible();
    await expect(canvas.getByText("In 20 min")).toBeVisible();
    await expect(await canvas.findByRole("heading", { level: 2, name: "Coming up" })).toBeVisible();
    // The events are not counted: still ten things need you.
    await expect(
      canvas.getByRole("heading", { level: 1, name: "10 things need you" }),
    ).toBeVisible();
  },
};

/** Next up selected: Join on 1, Open in Calendar on Enter. */
export const NextUpSelected: Story = {
  ...onDemoDay,
  args: { path: PATH, api: yourDay() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByText("Kitchen plan call with Marta Ferreira"));
    await expect(
      await canvas.findByRole("group", {
        name: "Actions for Kitchen plan call with Marta Ferreira",
      }),
    ).toBeVisible();
  },
};

/** "Add to my calendar" on the delivery only Sam sees: the toast says where, with Undo. */
export const AddToMyCalendar: Story = {
  ...onDemoDay,
  args: { path: PATH, api: yourDay() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByRole("button", {
        name: "Add to my calendar: Furniture delivery, 2 of 3 parcels",
      }),
    );
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText("Added to Work")).toBeVisible();
  },
};

/** The same day on a phone: the aside stacks under the list, nothing scrolls sideways. */
export const YourDayPhone: Story = {
  ...onDemoDay,
  args: { path: PATH, api: yourDay() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByRole("heading", { level: 2, name: "Coming up" });
    await expect(canvasElement.ownerDocument.documentElement.scrollWidth).toBeLessThanOrEqual(
      globalThis.innerWidth,
    );
  },
};

// Scout isn't sure whose move it is (campbooks-cloud#372): Ana Costa wrote to
// Águas do Ribatejo nine days ago, copying Sam. The model read a reply for Sam
// at 85%, short of the 90% a reply needs to count as sure.
function copied(): Record<string, MockRoute> {
  return home({
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [] } }
        : { body: { items: [COPIED_ITEM, ...ITEMS], counts: COUNTS } },
    [`GET ${W}/conversations`]: {
      body: { conversations: [COPIED, ...NEEDS, SOFIA], next_cursor: null },
    },
    [`GET ${W}/conversations/${COPIED.id}`]: { body: COPIED_DETAIL },
    [`POST ${W}/conversations/${COPIED.id}/need/resolve`]: { body: COPIED_WAITING },
    [`POST ${W}/conversations/${COPIED.id}/need/reopen`]: { body: COPIED },
    [`GET ${W}/drafts`]: { body: { drafts: [] } },
    [`POST ${W}/conversations/${COPIED.id}/nudge/draft`]: {
      body: {
        message_id: COPIED.need?.message_id,
        text: "Hello,\n\nCould you confirm the meter reading for the studio? Ana asked about it nine days ago.\n\nThanks,\nSam Rivera",
      },
    },
  });
}

const copiedPlay: Story["play"] = async ({ canvasElement }) => {
  const canvas = within(canvasElement);
  const line = await canvas.findByText("Ana Costa wrote to Águas do Ribatejo. You’re in Cc.");
  await expect(line).toBeVisible();
  await expect(
    canvas.getByText(
      "They haven’t answered in 9 days. Scout can’t tell whose move this is. What’s next for you?",
    ),
  ).toBeVisible();
  // Selected, its actions show under it.
  await userEvent.click(line);
  const actions = await canvas.findByRole("group", { name: "Actions for Ana Costa" });
  await expect(within(actions).getByRole("button", { name: /^Nudge Águas do Ribatejo…/u })).toBeVisible();
  await expect(within(actions).getByRole("button", { name: /^Reply to Ana…/u })).toBeVisible();
  await expect(within(actions).getByRole("button", { name: /^Nothing for me/u })).toBeVisible();
};

/** Scout isn't sure: who wrote to whom, that they are quiet, and the three ways to answer. */
export const ScoutIsntSure: Story = {
  args: { path: PATH, api: copied() },
  play: copiedPlay,
};

/** The same card on a phone: the buttons stack, nothing scrolls sideways. */
export const ScoutIsntSurePhone: Story = {
  args: { path: PATH, api: copied() },
  ...at("phone", 375, 812),
  play: async (context) => {
    await copiedPlay(context);
    await expect(
      context.canvasElement.ownerDocument.documentElement.scrollWidth,
    ).toBeLessThanOrEqual(globalThis.innerWidth);
  },
};

/** 1: the conversation waits on Águas do Ribatejo, and Scout offers the nudge to them. */
export const ScoutIsntSureNudge: Story = {
  args: { path: PATH, api: copied() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByText("Ana Costa wrote to Águas do Ribatejo. You’re in Cc."),
    );
    await userEvent.keyboard("1");
    const body = within(canvasElement.ownerDocument.body);
    const dialog = await body.findByRole("dialog", { name: "Nudge Águas" });
    // It rises in over a moment.
    await waitFor(async () => {
      await expect(dialog).toBeVisible();
    });
  },
};

// Twins for the recording (no play function): the same screens, left for the
// recording script to drive.
export const ScoutIsntSureRecording: Story = { args: { path: PATH, api: copied() } };

export const ScoutIsntSurePhoneRecording: Story = {
  args: { path: PATH, api: copied() },
  ...at("phone", 375, 812),
};

// Invitations awaiting Sam's answer (campbooks-cloud#360), on the same
// demo day: a webinar tomorrow from a service, while the furniture is
// delivered (busy then), and Beatriz's offsite planning on Thursday (free
// then).
function invited(): Record<string, MockRoute> {
  const schedule = scheduleApi([...HOME_EVENTS, ...HOME_INVITATION_EVENTS], {
    calendars: HOME_CALENDARS,
  });
  return {
    ...yourDay(),
    ...schedule.routes,
    [`GET ${W}/schedule/settings`]: { body: HOME_SETTINGS },
    [`GET ${W}/attention`]: (_body, url) =>
      url.searchParams.get("state") === "cleared"
        ? { body: { items: CLEARED, counts: [], tasks_elsewhere: 0 } }
        : {
            body: {
              items: HOME_ITEMS_INVITED,
              counts: HOME_COUNTS_INVITED,
              tasks_elsewhere: HOME_TASKS_ELSEWHERE,
            },
          },
  };
}

/** Each invitation is a Decide row: the day of its event, who asked, when and where, free or busy. */
export const Invitations: Story = {
  ...onDemoDay,
  args: { path: PATH, api: invited() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      await canvas.findByRole("heading", { level: 1, name: "12 things need you" }),
    ).toBeVisible();
    await expect(
      await canvas.findByText("Invitation: Offsite planning · Office, 3rd floor"),
    ).toBeVisible();
    await expect(canvas.getByText("Free then")).toBeVisible();
    await expect(canvas.getByText("Busy then")).toBeVisible();
    await expect(canvas.getByText("Event tomorrow")).toBeVisible();
  },
};

/** Selected, an invitation offers Accept (1), Decline (2), Maybe (3), Snooze and Open in Calendar. */
export const InvitationSelected: Story = {
  ...onDemoDay,
  args: { path: PATH, api: invited() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByText("Invitation: Offsite planning · Office, 3rd floor"),
    );
    const actions = await canvas.findByRole("group", { name: "Actions for Offsite planning" });
    await expect(within(actions).getByRole("button", { name: /Accept/u })).toBeVisible();
    await expect(within(actions).getByRole("button", { name: /Maybe/u })).toBeVisible();
  },
};

/** 1 accepts it: the row folds away, and the toast offers Undo. */
export const InvitationAccepted: Story = {
  ...onDemoDay,
  args: { path: PATH, api: invited() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(
      await canvas.findByText("Invitation: Offsite planning · Office, 3rd floor"),
    );
    await userEvent.keyboard("1");
    const body = within(canvasElement.ownerDocument.body);
    await expect(await body.findByText("Accepted: Offsite planning")).toBeVisible();
  },
};

/** The invitations on a phone: nothing scrolls sideways. */
export const InvitationsPhone: Story = {
  ...onDemoDay,
  args: { path: PATH, api: invited() },
  ...at("phone", 375, 812),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await canvas.findByText("Invitation: Offsite planning · Office, 3rd floor");
    await expect(canvasElement.ownerDocument.documentElement.scrollWidth).toBeLessThanOrEqual(
      globalThis.innerWidth,
    );
  },
};
