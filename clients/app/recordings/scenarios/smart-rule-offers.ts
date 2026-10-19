import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Smart-rule offers (campbooks-cloud#169) on the real stack, started as
// inbox.ts says, with the fake Google holding 40 more Design Notes digests
// (unread, a few hours apart over five days), so Scout notices them:
//
//   pnpm --dir clients fake-google --port 9071 --lookalikes 40
//   (the server, worker and Vite as in inbox.ts)
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record smart-rule-offers \
//     --base-url http://localhost:5181
//
// Report and delete on the phishing warning moves it to spam (with Undo) and
// offers "Never see emails like this again?"; yes opens the smart-rule
// confirmation, which alone saves, and the toast's Undo removes the rule.
// Right after the first group on Home, "Scout noticed" offers a rule for the
// digests: Archive them… / File under…, Something else… (the goal can be
// changed: Highlight them, Newsletters, Mark them read, or your own words),
// Not now (with Undo). Fictional person and mail only;
// nothing is sent.

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/**
 * Wait until Home shows `text` (the sync and the decide pass run first).
 * Reloads every few seconds: each load opens the change feed, which is
 * rate-limited.
 */
async function homeShows(page: Page, workspace: string, text: string | RegExp): Promise<void> {
  for (let attempt = 0; attempt < 24; attempt += 1) {
    await page.goto(`/w/${workspace}`);
    await page.getByRole("heading", { level: 1 }).waitFor();
    if ((await page.getByText(text).count()) > 0) {
      return;
    }
    await pause(4500);
  }
}

export default async function smartRuleOffers({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeShows(page, workspace, /This may be phishing/u);
  await caption(page, "A message that may be phishing waits in Review", 2200);

  await page
    .getByText(/This may be phishing/u)
    .first()
    .click();
  await caption(page, "Report and delete is 1: it goes to spam, at the provider too");
  await (phone
    ? page.getByRole("button", { name: /^Report and delete/u }).click()
    : page.keyboard.press("1"));
  const offer = page.getByRole("dialog", { name: /Never see emails like this again/u });
  await offer.waitFor();
  await caption(page, "Then Scout offers a smart rule for mail like it", 2400);

  await (phone
    ? offer.getByRole("button", { name: /^Yes, from now on/u }).click()
    : page.keyboard.press("1"));
  const confirm = page.getByRole("dialog", { name: /A rule for mail like this/u });
  await confirm.waitFor();
  await caption(
    page,
    "Nothing is saved yet: the smart rule, what it matches, where it lives",
    2800,
  );
  await (phone
    ? confirm.getByRole("button", { name: /^Save smart rule/u }).click()
    : page.keyboard.press("Meta+Enter"));
  await page.getByText("Smart rule saved").first().waitFor();
  await caption(page, "Saved, with Undo in the toast", 2000);
  await page.getByRole("button", { name: /^Undo/u }).first().click();
  await pause(1500);
  await caption(page, "Undo removes the rule again", 1800);

  // Scout's look at the inbox starts when Home asks; what it finds shows next.
  await homeShows(page, workspace, "Scout noticed");
  await page.getByText("Scout noticed").first().scrollIntoViewIfNeeded();
  await caption(page, "Right after the first group: something Scout noticed", 2400);
  const noticed = page.getByRole("region", { name: "Scout noticed" });
  await noticed.getByRole("button", { name: /^Something else/u }).click();
  const goal = page.getByRole("dialog", { name: /A rule for mail from/u });
  await goal.waitFor();
  await caption(page, "First question: what should happen to their mail? Nothing is picked", 2800);
  await goal.getByRole("radio", { name: /^Highlight them/u }).check({ force: true });
  await caption(
    page,
    "Highlight them: it waits on Home under Worth a look until opened once",
    3000,
  );
  await page.keyboard.press("Escape");
  await pause(600);
  await noticed.getByRole("button", { name: /^(Archive them|File under)/u }).click();
  await page.getByRole("dialog", { name: /A rule for mail from/u }).waitFor();
  await caption(page, "Scout's guess is picked when its own button opens it", 2400);
  await page.keyboard.press("Escape");
  await pause(600);
  await noticed.getByRole("button", { name: /^Not now/u }).click();
  await caption(page, "Not now: it asks another time, with Undo", 2000);
  await page.getByRole("button", { name: /^Undo/u }).first().click();
  await pause(1500);
  await caption(page, "Back again", 1500);
}
