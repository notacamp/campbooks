import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// The Sent view (campbooks-cloud#240), on the real stack as the `inbox`
// scenario runs it (see its header for the commands: the fake Google of
// `scripts/fake-provider.ts` with the demo Gmail of `scripts/fake-gmail.ts`).
// The demo mailbox holds two conversations Sam started: "Quote for the harbour
// workshop", which nobody answered (archived on arrival, so only Sent lists
// it), and "Can we move Thursday's call?", which Pedro answered (back in the
// inbox). Nothing is intercepted and nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record sent \
//     --base-url http://localhost:5181
//
// Record at 375px and on a desktop. Fictional people and addresses only
// (`.example`); no token is shown.

const QUOTE = "Quote for the harbour workshop";
const CALL = "Can we move Thursday's call?";

/** Desktop and phone differ only where the screens do: one pane or two. */
function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function sent({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  // The conversation Sam started is not in the inbox.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText(CALL).first().waitFor();
  await caption(page, "Mail you start stays out of the inbox until someone replies", 2400);

  // Sent: G then 5 on a desktop, the Sent chip on a phone.
  if (phone) {
    await page.getByRole("button", { name: "Sent", exact: true }).click();
  } else {
    await page.keyboard.press("g");
    await page.keyboard.press("5");
  }
  await page.getByRole("heading", { level: 1, name: "Sent" }).waitFor();
  const list = page.getByRole("region", { name: "Conversations" });
  await list.getByText("To Clara Nunes").first().waitFor();
  await caption(
    page,
    phone ? "Sent: the chip after the streams" : "Sent: G then 5 from anywhere",
    2200,
  );
  await caption(page, "Each row starts with who it went to", 2200);

  // The one someone answered says so.
  await list.getByText("Replied").first().waitFor();
  await caption(page, "“Replied” marks the one someone answered", 2400);

  // The unanswered one opens like any conversation, with the reply box pinned.
  await list.getByText(QUOTE).first().click();
  await page.getByRole("heading", { name: QUOTE }).waitFor();
  await pause(600);
  await caption(page, "It opens as usual, with the reply box at the bottom", 2400);
}
