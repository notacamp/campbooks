import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Search on the real stack, set up exactly like the `inbox` scenario (see its
// header: Vite dev server, the real server on a scratch database, the fake
// Google with its demo mailbox), then:
//
//   SEARCH_BASE_URL=http://localhost:5181 INBOX_BASE_URL=http://localhost:5181 \
//   pnpm --dir clients record search --base-url http://localhost:5181
//
// Desktop: `/` opens the overlay, typing shows instant rows with the matched
// words marked and the parsed filter as a chip, ⌘K hands its query over, ⌘↵
// opens the results page (newest first, "Most relevant" toggle, reader
// beside, inbox shortcuts), then the empty states. Phone: the top bar's
// search button, the full-screen page, wrapping chips, the reader as its own
// screen. Fictional mail only; no token is ever shown.

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

async function typeSlowly(page: Page, text: string): Promise<void> {
  await page.keyboard.type(text, { delay: 90 });
  // The overlay waits ~120 ms after the last key before asking.
  await pause(700);
}

export default async function search({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  const shots = process.env["INBOX_SHOTS"];
  const shot = async (name: string) => {
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/${name}-${phone ? "375" : "desktop"}.png` });
    }
  };

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText("Q4 plan: can you review by Friday?").first().waitFor();

  if (phone) {
    await caption(page, "Search is a button in the top bar");
    await page.getByRole("button", { name: "Search conversations" }).click();
    const field = page.getByRole("searchbox", { name: "Search conversations" });
    await field.waitFor();
    await caption(page, "An empty search shows the operators you can use", 1800);
    await shot("01-empty");
    await typeSlowly(page, "invoice from:cloudhost has:attachment");
    await page.getByRole("button", { name: /Cloudhost Billing/u }).waitFor();
    await caption(
      page,
      "The filters it understood are chips that wrap; the words that matched are marked",
      2400,
    );
    await shot("02-results");
    await page.getByRole("button", { name: "Remove filter: Has attachment" }).click();
    await pause(900);
    await caption(page, "Remove a chip and it searches again", 1600);
    await page.getByRole("button", { name: /Cloudhost Billing/u }).click();
    await page.getByRole("heading", { name: "Your invoice CH-20418 for €148.00" }).waitFor();
    await caption(page, "A result opens as its own screen, like the inbox", 2000);
    await shot("03-reader");
    await page.getByRole("button", { name: "Back to results" }).first().click();
    await page.getByRole("button", { name: /Cloudhost Billing/u }).waitFor();
    await caption(page, "Back keeps the search", 1400);
    await field.fill("zebra");
    await page.getByRole("heading", { name: /No conversations match/u }).waitFor();
    await caption(page, "When nothing matches it says so", 1800);
    await shot("04-nothing");
    return;
  }

  // ── desktop ─────────────────────────────────────────────────────────────
  await caption(page, "Press / to search from anywhere in a workspace");
  await page.keyboard.press("/");
  const field = page.getByRole("combobox", { name: "Search conversations" });
  await field.waitFor();
  await caption(page, "Nothing typed yet: the operators, and later your recent searches", 2000);
  await shot("01-overlay-empty");
  await typeSlowly(page, "invoice");
  await page.getByRole("option", { name: /Cloudhost Billing/u }).waitFor();
  await caption(page, "Instant rows by relevance: the matched words are marked", 2200);
  await shot("02-instant");
  await typeSlowly(page, " from:cloudhost");
  await page.getByRole("button", { name: "Remove filter: From cloudhost" }).waitFor();
  await caption(page, "The filter it understood is a chip you can remove", 2200);
  await shot("03-chip");
  await page.getByRole("button", { name: "Remove filter: From cloudhost" }).click();
  await pause(1000);
  await page.keyboard.press("ArrowDown");
  await caption(
    page,
    "Up and down move, Enter opens in the inbox reader, ⌘↵ shows all results",
    2600,
  );
  await shot("04-keys");

  // All results.
  await page.keyboard.press("Meta+Enter");
  await page.getByRole("region", { name: "Search results" }).waitFor();
  await page.getByRole("button", { name: /Cloudhost Billing/u }).waitFor();
  await caption(page, "The results page: newest first", 2000);
  await shot("05-results");
  await page.getByRole("button", { name: "Most relevant" }).click();
  await pause(1200);
  await caption(page, "Most relevant puts the best match first", 2000);
  await page.getByRole("button", { name: "Newest" }).click();
  await pause(900);

  // Reader beside the list, inbox shortcuts.
  await page.getByRole("button", { name: /Cloudhost Billing/u }).click();
  await page.getByRole("heading", { name: "Your invoice CH-20418 for €148.00" }).waitFor();
  await caption(page, "The reader opens beside the list", 2000);
  await shot("06-reader");
  await page.keyboard.press("j");
  await pause(900);
  await caption(page, "J and K move through the results, as in the inbox", 1800);
  await page.keyboard.press("e");
  await pause(900);
  await caption(page, "E archives it, with Undo", 1800);
  await shot("07-done");
  await page.keyboard.press("z");
  await pause(1200);
  await caption(page, "Z takes it back", 1400);

  // ⌘K hand-off.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.keyboard.press("Meta+k");
  await page.getByRole("combobox", { name: "Search, ask, or run a command" }).waitFor();
  await typeSlowly(page, "lease");
  await caption(page, "⌘K: the last row hands the query to search", 2000);
  await shot("08-command-menu");
  await page.getByRole("option", { name: /Search conversations for/u }).click();
  await page.getByRole("option", { name: /Lease for the studio/u }).waitFor();
  await caption(page, "…and search opens with it", 2000);
  await shot("09-handoff");
  await page.keyboard.press("Meta+Enter");

  // Recents, nothing found.
  await page.getByRole("region", { name: "Search results" }).waitFor();
  await page.keyboard.press("/");
  await caption(page, "On the page, / focuses the field", 1200);
  await page.keyboard.press("Meta+a");
  await page.keyboard.type("zebra", { delay: 90 });
  await page.getByRole("heading", { name: /No conversations match/u }).waitFor();
  await caption(page, "When nothing matches it says so", 2000);
  await shot("10-nothing");
  await page.keyboard.press("Meta+a");
  await page.keyboard.press("Backspace");
  await page.getByRole("heading", { name: "Recent searches" }).waitFor();
  await caption(page, "Recent searches stay on this device", 2000);
  await shot("11-recent");
}
