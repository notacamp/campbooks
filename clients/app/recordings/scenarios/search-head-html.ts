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
// sender), so finding it by that word shows its text was indexed. Then it
// is forwarded (never sent) to show the quote carries those words too.
// Every locator is scoped to the results, the reader or the composer.
// Fictional mail only.

const WORD = "harbourlight";
const SENDER = "Quay Office";
const SUBJECT = "Your berth for next season";

export default async function searchHeadHtml({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);

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

  // Forward it: the quote carries its words too (nothing is sent).
  await page.getByRole("button", { name: "Forward", exact: true }).last().click();
  const forward = page.getByRole("region", { name: "Forward" });
  await forward.waitFor();
  await forward.getByRole("button", { name: "Show quoted text" }).click();
  const quoted = forward.getByRole("textbox", { name: "Quoted text" });
  await quoted.waitFor();
  const quote = await quoted.inputValue();
  if (!quote.includes("harbourlight pontoon")) {
    throw new Error(`the forward does not quote the email's words: ${quote}`);
  }
  await caption(page, "Forward it: the quote carries its words, not just a header", 2600);
}
