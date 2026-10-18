import type { ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// A forward of mail whose words were not read in time still quotes them
// (campbooks-cloud#250). On the real stack set up as the `inbox` scenario
// says (its header), with one more variable on the server:
//
//   CAMPBOOKS_DEV_TEXT_DEADLINE_MS=0 … cargo run -p campbooks-server -- all
//   INBOX_BASE_URL=http://localhost:5181 \
//   pnpm --dir clients record forward-text-unread --base-url http://localhost:5181
//
// `CAMPBOOKS_DEV_TEXT_DEADLINE_MS` is read by debug builds only (release
// builds never look at it; it is off when unset). At 0, `conversations.get`
// has no time to read any HTML as text, so every message that came as HTML
// alone is marked `text_unread` and arrives without its words: the state a
// very large or slow email reaches in production. The demo Gmail
// (`scripts/fake-gmail.ts`) holds "Your berth for next season" from the
// Quay Office, HTML alone, saying "harbourlight pontoon". Opened, it shows
// its HTML; forwarded (never sent), the composer reads its words first and
// the quote carries them. Every locator is scoped to the reader or the
// composer. Fictional mail only.

const SENDER = "Quay Office";
const SUBJECT = "Your berth for next season";

export default async function forwardTextUnread({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await page.goto(`/w/${workspace}/inbox/primary`);
  const list = page.getByRole("region", { name: "Conversations" });
  await list.getByText(SUBJECT).first().click();
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: SUBJECT }).first().waitFor();
  await reader
    .locator(`iframe[title*="${SENDER}"]`)
    .last()
    .contentFrame()
    .getByText(/harbourlight pontoon/u)
    .waitFor();
  await caption(page, "This email came as HTML alone, too slow to read as text in time", 2600);

  // Forward it: the composer reads its words before quoting (nothing is sent).
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
  await caption(page, "Forward it: its words are read first, so the quote is never empty", 2800);
}
