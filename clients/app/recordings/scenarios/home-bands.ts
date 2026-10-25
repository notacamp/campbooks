import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Home by priority bands (campbooks-cloud#358): Now, Today, This week and
// Whenever, each row saying first what it is and why it is there, an opened
// item saying why it is here, Done and Undo, and the tasks that are in
// Tasks. Needs only the Vite dev server: the page's /bff calls are answered
// in the page from the demo data the screen stories use
// (`test/demo/home.ts`), on the demo's Tuesday 13 October 2026. The bands
// themselves are the attention module's (its tests).
//
//   pnpm --dir clients/app exec vite --port 5199
//   pnpm --dir clients record home-bands --base-url http://localhost:5199
//
// Fictional people and mail only. Nothing is sent.

/** The demo's clock: Tuesday 13 October 2026, 15:00 UTC (`test/demo/conversations.ts`). */
const NOW = new Date("2026-10-13T15:00:00Z");

/**
 * The page's stand-in for the BFF, installed before the app loads. A script
 * string: the recordings are type-checked without the DOM types. Done on
 * Tiago's reply takes it off the list until Undo puts it back.
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
  let done = false;
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : new Request(input, init);
    const url = new URL(request.url, location.href);
    if (!url.pathname.startsWith("/bff/")) return original(input, init);
    const [attention, conversations, workspaces, home] = await demo;
    const { CLEARED, SOFIA, TIAGO, TIAGO_DETAIL } = attention;
    const { INBOX, STREAM_COUNTS, W } = conversations;
    const { DUARTE, SESSION } = workspaces;
    const { HOME_COUNTS, HOME_ITEMS, HOME_NEEDS, HOME_TASKS, HOME_TASKS_ELSEWHERE } = home;
    const tiago = done ? { ...TIAGO, need: null } : TIAGO;
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
        const items = HOME_ITEMS.filter((item) => !(done && item.record.id === TIAGO.id));
        return json({ items, counts: HOME_COUNTS, tasks_elsewhere: HOME_TASKS_ELSEWHERE });
      }
      case "GET " + W + "/conversations":
        return json({
          conversations: [...HOME_NEEDS.map((c) => (c.id === TIAGO.id ? tiago : c)), SOFIA],
          next_cursor: null,
        });
      case "GET " + W + "/tasks":
        return json({ tasks: HOME_TASKS });
      case "GET " + W + "/rule-notices":
        return json({ notices: [] });
      case "GET " + W + "/drafts":
        return json({ drafts: [] });
      case "GET " + W + "/conversations/" + TIAGO.id:
        return json({ ...TIAGO_DETAIL, conversation: tiago });
      case "POST " + W + "/conversations/" + TIAGO.id + "/need/resolve":
        done = true;
        return json({ ...TIAGO, need: null });
      case "POST " + W + "/conversations/" + TIAGO.id + "/need/reopen":
        done = false;
        return json(TIAGO);
      default:
        return json({ status: 404, code: "not_found", title: "Not found" }, 404);
    }
  };
})()`;

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Bring the band titled `name` into view and linger on it. */
async function showBand(page: Page, name: RegExp, ms: number): Promise<void> {
  await page.getByRole("heading", { level: 2, name }).scrollIntoViewIfNeeded();
  await pause(ms);
}

export default async function homeBands({ page }: ScenarioContext): Promise<void> {
  await page.clock.setFixedTime(NOW);
  await page.addInitScript(FAKE_BFF);
  const phone = isPhone(page);
  // The demo workspace's id, as `test/demo/workspaces.ts` has it.
  const workspace = await page
    .goto("/")
    .then(() => page.evaluate(`import("/test/demo/workspaces.ts").then((demo) => demo.DUARTE.id)`));
  await page.goto(`/w/${String(workspace)}`);
  await page.getByText("Tiago has waited 4 days").first().waitFor();
  await caption(page, "Demo workspace, fictional mail", 1600);
  await caption(page, "Home by priority, whatever the kind: Now, Today, This week, Whenever", 2600);
  await caption(page, "Each row says first what it is and why it is there", 2400);

  const tiago = page.getByText("Tiago Nunes is waiting for your reply").first();
  if (phone) {
    for (const band of [/^Today/u, /^This week/u, /^Whenever/u]) {
      await showBand(page, band, 1500);
    }
    await page
      .getByText(/more in Tasks/u)
      .first()
      .scrollIntoViewIfNeeded();
    await caption(page, "Only today's tasks; the others are in Tasks", 2200);
    await showBand(page, /^Today/u, 900);
    await tiago.click();
  } else {
    await caption(page, "↓ moves down the list, band after band", 1200);
    // The warning is selected; five steps reach Tiago, opening each row.
    for (let step = 0; step < 5; step += 1) {
      await page.keyboard.press("ArrowDown");
      await pause(700);
    }
  }
  const why = page.getByRole("region", { name: "Why it’s here" });
  await why.waitFor();
  await why.scrollIntoViewIfNeeded();
  await caption(page, "Opened: why it is here, and what the band holds", 2800);

  // Done: the row folds away, and Undo brings it back.
  await (phone
    ? page
        .getByRole("group", { name: "Actions for Tiago Nunes" })
        .getByRole("button", { name: /^Done/u })
        .click()
    : page.keyboard.press("e"));
  await caption(page, "Done: Tiago's reply leaves the list", 2000);
  const undo = page.getByRole("button", { name: /^Undo/u }).first();
  await undo.waitFor();
  await (phone ? undo.click() : page.keyboard.press("z"));
  await tiago.waitFor();
  await caption(page, "Undo puts it back where it was", 2000);

  if (!phone) {
    await page
      .getByText(/more in Tasks/u)
      .first()
      .scrollIntoViewIfNeeded();
    await caption(page, "Only today's tasks on Home; G then T opens the rest", 2400);
  }
  await pause(800);
}
