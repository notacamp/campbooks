import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Your calendar on Home (campbooks-cloud#359): Next up above Now with Join,
// today's events at the end of Today on the same keys, Enter opening one in
// Calendar, and Coming up's "Add to my calendar" with Undo. Needs only the
// Vite dev server: the page's /bff calls are answered in the page from the
// demo data the screen stories use (`test/demo/home.ts`), on the demo's
// Tuesday 13 October 2026, 15:00 UTC: 11:00 on Sam's New York calendar.
//
//   pnpm --dir clients/app exec vite --port 5199
//   pnpm --dir clients record home-calendar --base-url http://localhost:5199
//
// Fictional people, mail and events only. Nothing is sent.

/** The demo's clock: Tuesday 13 October 2026, 15:00 UTC (`test/demo/conversations.ts`). */
const NOW = new Date("2026-10-13T15:00:00Z");

/**
 * The page's stand-in for the BFF, installed before the app loads. A script
 * string: the recordings are type-checked without the DOM types. Adding the
 * delivery to Work puts it on the provider until Undo takes it back off.
 */
const FAKE_BFF = `(() => {
  const original = window.fetch.bind(window);
  const demo = Promise.all([
    import("/test/demo/attention.ts"),
    import("/test/demo/conversations.ts"),
    import("/test/demo/workspaces.ts"),
    import("/test/demo/home.ts"),
  ]);
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const json = (body, status = 200) => Response.json(body, { status });
  let pushed = false;
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const url = new URL(request.url, location.href);
    if (!url.pathname.startsWith("/bff/")) return original(input, init);
    const [attention, conversations, workspaces, home] = await demo;
    const { CLEARED, MARTA, MARTA_DETAIL, SOFIA } = attention;
    const { INBOX, STREAM_COUNTS, W } = conversations;
    const { DUARTE, SESSION } = workspaces;
    const {
      HOME_CALENDARS, HOME_COUNTS, HOME_DELIVERY, HOME_DUE, HOME_EVENTS, HOME_ITEMS,
      HOME_NEEDS, HOME_SETTINGS, HOME_TASKS, HOME_TASKS_ELSEWHERE,
    } = home;
    const S = W + "/schedule";
    const delivery = pushed ? { ...HOME_DELIVERY, on_provider: true } : HOME_DELIVERY;
    const route = request.method + " " + url.pathname;
    await sleep(60);
    switch (route) {
      case "GET /bff/session":
        return json(SESSION);
      case "GET /bff/api/v1/workspaces":
        return json({ workspaces: [DUARTE] });
      case "GET " + W + "/inboxes":
        return json({ inboxes: [INBOX] });
      case "GET " + W + "/streams":
        return json({ streams: STREAM_COUNTS });
      case "GET " + W + "/attention": {
        const state = url.searchParams.get("state");
        if (state === "cleared") return json({ items: CLEARED, counts: [], tasks_elsewhere: 0 });
        if (state === "waiting") return json({ items: [], counts: [], tasks_elsewhere: 0 });
        return json({ items: HOME_ITEMS, counts: HOME_COUNTS, tasks_elsewhere: HOME_TASKS_ELSEWHERE });
      }
      case "GET " + W + "/conversations":
        return json({ conversations: [...HOME_NEEDS, SOFIA], next_cursor: null });
      case "GET " + W + "/conversations/" + MARTA.id:
        return json(MARTA_DETAIL);
      case "GET " + W + "/tasks":
        return json({ tasks: [...HOME_TASKS, HOME_DUE] });
      case "GET " + W + "/rule-notices":
        return json({ notices: [] });
      case "GET " + W + "/drafts":
        return json({ drafts: [] });
      case "GET " + S + "/settings":
        return json(HOME_SETTINGS);
      case "GET " + S + "/calendars":
        return json({ calendars: HOME_CALENDARS });
      case "GET " + S + "/events":
        return json({
          events: HOME_EVENTS.map((each) => (each.id === delivery.id ? delivery : each)),
          truncated: false,
        });
      case "POST " + S + "/events/" + HOME_DELIVERY.id + "/push":
        pushed = true;
        return json({ ...HOME_DELIVERY, on_provider: true });
      case "DELETE " + S + "/events/" + HOME_DELIVERY.id + "/push":
        pushed = false;
        return json({ event: HOME_DELIVERY, id: HOME_DELIVERY.id, calendar_id: HOME_CALENDARS[1].id });
      default:
        return json({ status: 404, code: "not_found", title: "Not found" }, 404);
    }
  };
})()`;

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

const CALL = "Kitchen plan call with Marta Ferreira";
const DELIVERY = "Add to my calendar: Furniture delivery, 2 of 3 parcels";

/** Coming up's "Add to my calendar" on the delivery, then Undo. */
async function addAndUndo(page: Page, phone: boolean): Promise<void> {
  const coming = page.getByRole("heading", { level: 2, name: "Coming up" });
  await coming.scrollIntoViewIfNeeded();
  await caption(page, "Coming up: the next 7 days, tasks due included", 2400);
  await caption(page, "A ring: only you see it, read from your mail", 2400);
  await page.getByRole("button", { name: DELIVERY }).click();
  await caption(page, "Add to my calendar: on Work, with Undo", 2400);
  const undo = page.getByRole("button", { name: /^Undo/u }).first();
  await undo.waitFor();
  await (phone ? undo.click() : page.keyboard.press("z"));
  await page.getByRole("button", { name: DELIVERY }).waitFor();
  await caption(page, "Undo takes it back off", 2000);
}

export default async function homeCalendar({ page }: ScenarioContext): Promise<void> {
  await page.clock.setFixedTime(NOW);
  await page.addInitScript(FAKE_BFF);
  const phone = isPhone(page);
  // The demo workspace's id, as `test/demo/workspaces.ts` has it.
  const workspace = await page
    .goto("/")
    .then(() => page.evaluate(`import("/test/demo/workspaces.ts").then((demo) => demo.DUARTE.id)`));
  await page.goto(`/w/${String(workspace)}`);
  const call = page.getByText(CALL, { exact: true }).first();
  await call.waitFor();
  await page.getByRole("heading", { level: 2, name: "Coming up" }).waitFor();
  await caption(page, "Demo workspace, fictional mail and calendar", 1600);
  await caption(page, "Your calendar joins Home", 2000);
  await caption(page, "Next up: your next event in the next two hours", 2400);
  await call.click();
  await caption(page, "Join opens the call; Enter opens it in Calendar", 2600);
  await caption(page, "The hero says what is next, from your calendar", 2400);

  const lunch = page.getByText("Lunch with Rui", { exact: true }).first();
  if (phone) {
    await page.getByRole("heading", { level: 2, name: /^Today/u }).scrollIntoViewIfNeeded();
    await lunch.scrollIntoViewIfNeeded();
    await caption(page, "The rest of today closes Today, never counted", 2600);
    await lunch.click();
    await page
      .getByRole("group", { name: "Actions for Lunch with Rui" })
      .getByRole("button", { name: /Open in Calendar/u })
      .click();
  } else {
    await caption(page, "↓ walks the list: Now, then Today", 1600);
    // Through Now and Today's items, onto lunch.
    const lunchRow = page.locator("button[data-selected]", { hasText: "Lunch with Rui" });
    for (let step = 0; step < 12 && (await lunchRow.count()) === 0; step += 1) {
      await page.keyboard.press("ArrowDown");
      await pause(500);
    }
    await caption(page, "Today's events close Today, never counted", 2600);
    await page.keyboard.press("Enter");
  }
  await page.getByRole("heading", { name: "Lunch with Rui" }).first().waitFor();
  await caption(page, "Enter opens it in Calendar, on its day", 2400);
  await page.goBack();
  await call.waitFor();
  await pause(600);

  await addAndUndo(page, phone);
  await pause(800);
}
