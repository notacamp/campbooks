import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Asks that lapse or go stale (campbooks-cloud#275) on the stack the
// `reply-dialog` scenario describes (inbox.ts's settings, a `decide` model
// and the fake `complete` model, which reads the sign-in link's lapse time),
// with the fake Google's `--stale-asks` demo mail. Fictional person and mail
// only.
//
//   pnpm --dir clients fake-google --port 9071 --stale-asks
//   pnpm --dir clients fake-complete
//   CAMPBOOKS_AI__COMPLETE__BASE_URL=http://127.0.0.1:9060/v1 \
//   CAMPBOOKS_AI__COMPLETE__MODEL=fake-complete … cargo run -p campbooks-server -- all
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record stale-asks \
//     --base-url http://localhost:5181
//
// Pedro's request is sent just inside the 14-day needs window: a few
// minutes after the sync, the clock (`conversations.wake_needs`, every
// minute) asks again whether it is still to do. The sign-in link was valid
// for ten minutes two hours ago: it never reaches Home, and the inbox says
// it expired.

const STILL = "Pedro Alves asked 2 weeks ago. Still to do?";

/** Reload Home until `text` shows, for at most `tries` seconds. */
async function homeShows(page: Page, workspace: string, text: string, tries = 420): Promise<void> {
  for (let attempt = 0; attempt < tries; attempt += 1) {
    await page.goto(`/w/${workspace}`);
    await page.getByRole("heading", { level: 1 }).waitFor();
    if ((await page.getByText(text).count()) > 0) {
      return;
    }
    await pause(1000);
  }
  throw new Error(`Home never showed “${text}”`);
}

export default async function staleAsks({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await homeShows(page, workspace, "Pedro Alves");
  await caption(page, "Pedro asked for the studio keys two weeks ago, with no date", 2400);
  await homeShows(page, workspace, STILL);
  await caption(page, "Two weeks on, Home asks again: still to do?", 2400);
  await page.getByText(STILL).click();
  await caption(page, "Yes keeps it as yours for good; No says it is no longer needed", 2400);
  await page.keyboard.press("1");
  await page.getByText("Thanks · Pedro Alves stays on your list").waitFor();
  await caption(page, "Yes: it stays, and Campbooks will not ask again", 2000);

  await caption(
    page,
    "The sign-in link was valid for 10 minutes after it was sent: it never reached Home",
    2400,
  );
  await page.goto(`/w/${workspace}/inbox/updates`);
  const link = page.getByText("Your Brightloop sign-in link").first();
  await link.waitFor();
  await caption(page, "In the inbox it says Expired, with nothing to act on", 2400);
  await link.click();
  await page.getByText("Expired").first().waitFor();
  await caption(page, "Scout's summary says how long it was valid, never “expires soon”", 2400);
}
