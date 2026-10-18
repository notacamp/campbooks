import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Search finds mail whose HTML has no <body> tag (campbooks-cloud#218), on
// the real stack set up as the `inbox` scenario says (its header), then:
//
//   INBOX_BASE_URL=http://localhost:5181 \
//   pnpm --dir clients record search-head-html --base-url http://localhost:5181
//
// The demo Gmail (`scripts/fake-gmail.ts`) holds "Your berth for next season"
// from the Quay Office: HTML alone, opening with `<head><meta …>` and never
// saying `<body>`. Only its text says "harbourlight" (not its subject or its
// sender), so finding it by that word shows its text was indexed. Every
// locator is scoped to the results or the reader. Fictional mail only.

const WORD = "harbourlight";
const SENDER = "Quay Office";
const SUBJECT = "Your berth for next season";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function searchHeadHtml({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  // The search index is built in the background after the mailbox syncs:
  // ask again until the message is found.
  const results = page.getByRole("region", { name: "Search results" });
  const row = results.getByRole("button", { name: new RegExp(SENDER, "u") });
  for (let attempt = 0; attempt < 30; attempt += 1) {
    await page.goto(`/w/${workspace}/search?q=${WORD}`);
    await results.waitFor();
    try {
      await row.first().waitFor({ timeout: 3000 });
      break;
    } catch {
      await pause(1000);
    }
  }
  await caption(page, `Search for “${WORD}”: a word only the email's text holds`, 2400);
  await caption(page, "Found, although its HTML has no body tag", 2200);

  await row.first().click();
  await page.getByRole("heading", { name: SUBJECT }).first().waitFor();
  const text = page
    .locator(`iframe[title*="${SENDER}"]`)
    .last()
    .contentFrame()
    .getByText(/harbourlight pontoon/u);
  await text.waitFor();
  await caption(page, "It opens with its text, the word in it", 2400);

  if (phone) {
    await page.getByRole("button", { name: "Back to results" }).first().click();
    await row.first().waitFor();
    await caption(page, "Back keeps the search", 1400);
  }
}
