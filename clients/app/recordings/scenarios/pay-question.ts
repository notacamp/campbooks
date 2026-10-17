import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Pay as "Is it paid?" (campbooks-cloud#167) on the real stack: start it as
// inbox.ts says, with a `decide` model configured (CAMPBOOKS_AI__DECIDE__*,
// Jev in production), since the money question is the model's. The demo
// mailbox holds Cloudhost's invoice (a bill) and Harbourline's receipt (a
// payment already made). Fictional person and mail only; nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record pay-question \
//     --base-url http://localhost:5181

const BILL = "Cloudhost Billing";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Wait until Home asks about the bill (the model pass runs after the sync). */
async function billAsked(page: Page, workspace: string): Promise<void> {
  for (let attempt = 0; attempt < 90; attempt += 1) {
    await page.goto(`/w/${workspace}`);
    await page.getByRole("heading", { level: 1 }).waitFor();
    if ((await page.getByText(/Is it paid\?/u).count()) > 0) {
      return;
    }
    await pause(1000);
  }
}

export default async function payQuestion({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await billAsked(page, workspace);
  await caption(page, "Campbooks can't see your bank, so a bill arrives as a question", 2600);
  await caption(page, "Harbourline's receipt confirms a payment: it isn't in Pay", 2400);

  await page.getByText(BILL).first().click();
  await caption(page, "Already paid is 1, Still to pay is 2");

  if (isPhone(page)) {
    await page.getByRole("button", { name: /^Still to pay/u }).click();
  } else {
    await page.keyboard.press("2");
  }
  await page.getByText(`Pay ${BILL}`).waitFor();
  await caption(page, "Still to pay: it stays in Pay, with a reminder as its first action", 2600);
  await caption(page, "The reminder comes from the due date only; with none, Monday");

  if (isPhone(page)) {
    await page.getByRole("button", { name: /^Mark paid/u }).click();
  } else {
    await page.keyboard.press("2");
  }
  await caption(page, "Mark paid: a one-line confirmation, and Undo in the toast", 2200);
  await page.getByRole("button", { name: /^Undo/u }).first().click();
  await pause(1500);
  await caption(page, "Undo brings back the bill you said is still to pay", 2000);
}
