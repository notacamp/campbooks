import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Rows leaving the inbox list (campbooks-cloud#289), in Storybook with demo
// data only: a mailbox that keeps what is done to it.
//   pnpm --dir clients storybook
//   pnpm --dir clients record inbox-rows-leave --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

const Q4 = "Q4 plan: can you review by Friday?";
const LEDGER = "Five tax dates before year end";
const PRIZE = "Claim your prize now!!!";

export default async function inboxRowsLeave({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const list = page.getByRole("region", { name: "Conversations" });

  await page.goto(story(phone ? "rows-leave-phone" : "rows-leave-idle"));
  await list.getByText(Q4).first().waitFor();
  await caption(page, "Primary. Watch the list as rows leave it", 2000);

  if (phone) {
    // Open the first conversation, Done (E): back to the list as its row folds away.
    await list.getByText(Q4).first().click();
    await page
      .getByRole("main", { name: "Conversation" })
      .getByRole("heading", { name: Q4 })
      .waitFor();
    await pause(800);
    await page.keyboard.press("e");
    await caption(page, "Done: the row says so, then folds away", 2200);
  } else {
    await page.keyboard.press("j");
    await pause(600);
    await page.keyboard.press("e");
    await caption(
      page,
      "Done (E): the row says so, then folds away and the rows below close up",
      2600,
    );
  }

  // Report spam on the prize mail: in red.
  for (let index = 0; index < 8; index += 1) {
    const cursor = page.locator("[data-selected]");
    const selected = (await cursor.count()) === 0 ? "" : await cursor.textContent();
    if (selected?.includes(PRIZE) === true) {
      break;
    }
    await page.keyboard.press("j");
    await pause(250);
  }
  await pause(500);
  await page.keyboard.press("!");
  await caption(page, "Report spam (!): said in red, then gone", 2400);

  // Delete, then Undo: the row comes back where it was.
  await page.keyboard.press("k");
  await pause(500);
  await page.keyboard.press("#");
  await caption(page, "Delete (#): the same, in red", 2200);
  await page.keyboard.press("z");
  await caption(page, "Undo (Z) puts it back where it was", 2200);

  if (!phone) {
    // A move to another stream: the row says where it went.
    await list.getByText(LEDGER).first().click();
    const reader = page.getByRole("main", { name: "Conversation" });
    await reader.getByRole("heading", { name: LEDGER }).waitFor();
    await pause(800);
    await reader
      .getByRole("button", { name: /^Move to Newsletters/u })
      .first()
      .click();
    await caption(page, "Moved to another stream: the row says where it went", 2600);
  }
  await pause(800);
}
