import type { ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// A long HTML email's quote says it holds only the first part
// (campbooks-cloud#255). On the real stack set up as the `inbox` scenario
// says (its header), with no extra variable:
//
//   INBOX_BASE_URL=http://localhost:5181 \
//   pnpm --dir clients record quote-long-html --base-url http://localhost:5181
//
// The demo Gmail (`scripts/fake-gmail.ts`) holds "Tide tables for the
// season" from the Quay Office: HTML alone, about 280 KiB, past the 256 KiB a
// quote is read from. It is read in time (not `text_unread`), and the reader
// shows the HTML whole, but a reply or forward quotes only its first part:
// the composer and the reply dialog say so. Nothing is sent. Every locator is
// scoped to the reader, the composer or the dialog. Fictional mail only.

const SUBJECT = "Tide tables for the season";
const COMPOSER_NOTE = "This email is long: the quote holds its first part.";
const DIALOG_NOTE = "This email is long: your reply quotes its first part.";

export default async function quoteLongHtml({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await page.goto(`/w/${workspace}/inbox/primary`);
  const list = page.getByRole("region", { name: "Conversations" });
  await list.getByText(SUBJECT).first().click();
  const reader = page.getByRole("main", { name: "Conversation" });
  await reader.getByRole("heading", { name: SUBJECT }).first().waitFor();
  await caption(page, "A long email that came as HTML alone: the reader shows all of it", 2600);

  // Reply: the dialog's quote says the reply quotes its first part.
  await page.getByRole("button", { name: "Reply", exact: true }).last().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByText(DIALOG_NOTE).waitFor();
  await caption(page, "Reply: the quote says only its first part goes with the reply", 2600);
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });

  // Forward: the composer says the same beside the quoted text (nothing is sent).
  await page.getByRole("button", { name: "Forward", exact: true }).last().click();
  const forward = page.getByRole("region", { name: "Forward" });
  await forward.waitFor();
  await forward.getByText(COMPOSER_NOTE).waitFor();
  await forward.getByRole("button", { name: "Show quoted text" }).click();
  const quoted = forward.getByRole("textbox", { name: "Quoted text" });
  await quoted.waitFor();
  const quote = await quoted.inputValue();
  if (!quote.includes("tide tables at the quay")) {
    throw new Error(`the forward does not quote the email's words: ${quote.slice(0, 200)}`);
  }
  await caption(page, "Forward: the composer says the quote holds the email's first part", 2800);
}
