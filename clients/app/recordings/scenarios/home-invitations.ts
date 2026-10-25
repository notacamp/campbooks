import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Invitations awaiting your answer on Home (campbooks-cloud#360): each one
// a Decide row in its band by the day of the event, with who asked, when
// and where, and whether you are free then; Accept on 1 with Undo in the
// toast, Decline on 2. Needs only the Vite dev server: the page's /bff calls
// are answered in the page from the demo data the screen stories use
// (`test/demo/home.ts`), on the demo's Tuesday 13 October 2026, 15:00 UTC:
// 11:00 on Sam's New York calendar.
//
//   pnpm --dir clients/app exec vite --port 5199
//   pnpm --dir clients record home-invitations --base-url http://localhost:5199
//
// Fictional people, mail and events only. Nothing is sent.

/** The demo's clock: Tuesday 13 October 2026, 15:00 UTC (`test/demo/conversations.ts`). */
const NOW = new Date("2026-10-13T15:00:00Z");

/**
 * The page's stand-in for the BFF, installed before the app loads. A script
 * string: the recordings are type-checked without the DOM types. An answer
 * takes the invitation off the list, as the server's projection does, and
 * "not answered" (Undo) puts it back.
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
  const answers = {};
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const url = new URL(request.url, location.href);
    if (!url.pathname.startsWith("/bff/")) return original(input, init);
    const [attention, conversations, workspaces, home] = await demo;
    const { CLEARED, MARTA, MARTA_DETAIL, SOFIA } = attention;
    const { INBOX, STREAM_COUNTS, W } = conversations;
    const { DUARTE, SESSION } = workspaces;
    const {
      HOME_CALENDARS, HOME_COUNTS_INVITED, HOME_DUE, HOME_EVENTS, HOME_INVITATION_EVENTS,
      HOME_ITEMS_INVITED, HOME_NEEDS, HOME_SETTINGS, HOME_TASKS, HOME_TASKS_ELSEWHERE,
    } = home;
    const S = W + "/schedule";
    const answered = (id) => answers[id] ?? "needs_action";
    const events = [...HOME_EVENTS, ...HOME_INVITATION_EVENTS].map((each) =>
      HOME_INVITATION_EVENTS.some((invite) => invite.id === each.id)
        ? { ...each, response: answered(each.id) }
        : each,
    );
    const route = request.method + " " + url.pathname;
    await sleep(60);
    const respond = route.match(/^POST .*\\/schedule\\/events\\/([^/]+)\\/response$/);
    if (respond) {
      const id = respond[1];
      const before = answered(id);
      const body = await request.json();
      answers[id] = body.response;
      const event = events.find((each) => each.id === id);
      return json({
        event: { ...event, response: body.response, pending: body.response !== "needs_action" },
        previous_response: before,
      });
    }
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
        const items = HOME_ITEMS_INVITED.filter(
          (item) => item.record.type !== "schedule.event" || answered(item.record.id) === "needs_action",
        );
        return json({ items, counts: HOME_COUNTS_INVITED, tasks_elsewhere: HOME_TASKS_ELSEWHERE });
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
        return json({ events, truncated: false });
      default:
        return json({ status: 404, code: "not_found", title: "Not found" }, 404);
    }
  };
})()`;

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

const OFFSITE = "Invitation: Offsite planning";
const WEBINAR = "Invitation: Cash flow basics webinar";

/** Select the row holding `text` with the keys, from wherever the selection is. */
async function walkTo(page: Page, text: string, key: "ArrowDown" | "ArrowUp"): Promise<void> {
  const row = page.locator("button[data-selected]", { hasText: text });
  for (let step = 0; step < 16 && (await row.count()) === 0; step += 1) {
    await page.keyboard.press(key);
    await pause(450);
  }
}

export default async function homeInvitations({ page }: ScenarioContext): Promise<void> {
  await page.clock.setFixedTime(NOW);
  await page.addInitScript(FAKE_BFF);
  const phone = isPhone(page);
  // The demo workspace's id, as `test/demo/workspaces.ts` has it.
  const workspace = await page
    .goto("/")
    .then(() => page.evaluate(`import("/test/demo/workspaces.ts").then((demo) => demo.DUARTE.id)`));
  await page.goto(`/w/${String(workspace)}`);
  const offsite = page.getByText(OFFSITE, { exact: true }).first();
  await offsite.waitFor();
  await caption(page, "Demo workspace, fictional mail and calendar", 1600);
  await caption(page, "Invitations you have not answered are on Home", 2200);

  if (phone) {
    await offsite.scrollIntoViewIfNeeded();
    await caption(page, "This week: Decide · Event Thu, who asked, when and where", 2800);
    await caption(page, "And whether you are free then", 2000);
    await offsite.click();
    await caption(page, "Accept, Decline or Maybe, right here", 2200);
    await page
      .getByRole("group", { name: "Actions for Offsite planning" })
      .getByRole("button", { name: /^Accept/u })
      .click();
    await page.getByText("Accepted: Offsite planning").first().waitFor();
    await caption(page, "Accepted, with Undo before it reaches Beatriz", 2400);
    await page.getByRole("button", { name: /^Undo/u }).first().click();
    await offsite.waitFor();
    await caption(page, "Undo: it is back, unanswered", 2200);

    const webinar = page.getByText(WEBINAR, { exact: true }).first();
    await webinar.scrollIntoViewIfNeeded();
    await caption(page, "Tomorrow's webinar: you have the delivery then", 2600);
    await webinar.click();
    await page
      .getByRole("group", { name: "Actions for Cash flow basics webinar" })
      .getByRole("button", { name: /^Decline/u })
      .click();
  } else {
    await caption(page, "↓ walks the list onto an invitation", 1800);
    await walkTo(page, OFFSITE, "ArrowDown");
    await caption(page, "This week: Decide · Event Thu, who asked, when and where", 2800);
    await caption(page, "You’re free then: nothing else on your calendar", 2400);
    await caption(page, "Accept 1, Decline 2, Maybe 3", 2000);
    await page.keyboard.press("1");
    await page.getByText("Accepted: Offsite planning").first().waitFor();
    await caption(page, "Accepted, with Undo before it reaches Beatriz", 2400);
    await page.keyboard.press("z");
    await offsite.waitFor();
    await caption(page, "Z takes it back: unanswered again", 2200);

    await walkTo(page, WEBINAR, "ArrowUp");
    await caption(page, "Tomorrow's webinar: you have the delivery then", 2600);
    await page.keyboard.press("2");
  }
  await page.getByText("Declined: Cash flow basics webinar").first().waitFor();
  await caption(page, "Declined: it leaves Home", 2400);
  await pause(800);
}
