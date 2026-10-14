import { readFileSync, writeFileSync } from "node:fs";

import type { Cookie, Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Home's "What needs you" filling in from mail stored before the upgrade
// (the needs backfill), on the real stack (see inbox.ts for how to start
// it). Fictional person and mail only. Two phases, one server restart:
//
// 1. Start the server as inbox.ts says, plus
//    CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS=0: mail is stored and read
//    by the rules, but no need is raised, as before attention existed. Then
//      NEEDS_BACKFILL_PHASE=setup NEEDS_BACKFILL_STATE=/tmp/needs-backfill \
//      INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record needs-backfill \
//        --base-url http://localhost:5181
//    signs up and syncs the demo mailbox, once per width.
// 2. Stop the server, run the seed of
//    migrations/<…>_conversations__needs_backfill.sql (what the upgrade
//    does: it lists the inboxes) on the scratch database, start the server
//    again without the variable, and record:
//      NEEDS_BACKFILL_PHASE=record NEEDS_BACKFILL_STATE=/tmp/needs-backfill \
//      INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record needs-backfill \
//        --base-url http://localhost:5181

interface Saved {
  workspace: string;
  cookies: Cookie[];
}

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Where this width's session is kept between the two phases. */
function stateFile(page: Page): string {
  const base = process.env["NEEDS_BACKFILL_STATE"] ?? "/tmp/needs-backfill";
  return `${base}-${isPhone(page) ? "375" : "desktop"}.json`;
}

/** The heading of Home, as shown. */
async function heading(page: Page): Promise<string> {
  const title = page.getByRole("heading", { level: 1 });
  await title.waitFor();
  return (await title.textContent()) ?? "";
}

export default async function needsBackfill({ page }: ScenarioContext): Promise<void> {
  if (process.env["NEEDS_BACKFILL_PHASE"] === "setup") {
    const workspace = await signedIn(page);
    const saved: Saved = { workspace, cookies: await page.context().cookies() };
    writeFileSync(stateFile(page), JSON.stringify(saved));
    return;
  }

  const saved = JSON.parse(readFileSync(stateFile(page), "utf8")) as Saved;
  await page.context().addCookies(saved.cookies);
  const home = `/w/${saved.workspace}`;
  await page.goto(home);
  await heading(page);
  await caption(page, "Mail synced before the upgrade: Home has nothing yet", 2400);
  await caption(
    page,
    "The upgraded server lists each inbox and works through it in the background",
  );

  // The dispatcher ticks every minute; the page reloads until Home fills.
  for (let attempt = 0; attempt < 90; attempt += 1) {
    if ((await heading(page)).includes("need you")) {
      break;
    }
    await pause(1000);
    await page.goto(home);
  }
  await caption(page, "What needs you, worked out from what Campbooks had already read", 2600);
  await caption(page, "No AI call: the stored answers, the same rules as for new mail", 2400);
  await caption(page, "Only recent mail: anything older than two weeks stays off Home", 2400);
  if (!isPhone(page)) {
    await page.keyboard.press("j");
    await pause(600);
    await page.keyboard.press("j");
    await pause(600);
  }
  await page.mouse.wheel(0, 600);
  await pause(1500);
  await caption(page, "Replied, archived, spam and trashed mail raises nothing", 2400);
}
