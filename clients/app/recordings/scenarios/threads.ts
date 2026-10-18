import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// Threads read as a back and forth (campbooks-cloud#239), on the real stack as
// the `inbox` scenario runs it (see its header for the commands: the fake
// Google of `scripts/fake-provider.ts` with the demo Gmail of
// `scripts/fake-gmail.ts`). The demo mailbox holds Clara's seven-message
// "Kitchen shelving quote", every reply quoting the thread and one answered
// inline, and Leo's "Photos from Tuesday's shoot", where Sam wrote last.
// Nothing is intercepted and nothing is sent: the composer is discarded.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record threads \
//     --base-url http://localhost:5181
//
// Record at 375px and on a desktop. Fictional people and addresses only
// (`.example`); no token is shown.

const KITCHEN = "Kitchen shelving quote";
const PHOTOS = "Photos from Tuesday's shoot";

/** Desktop and phone differ only where the screens do: one pane or two. */
function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Scroll what the reader scrolls: its pane on a desktop, the page on a phone. */
async function scroll(page: Page, phone: boolean, by: number): Promise<void> {
  if (!phone) {
    await page.getByRole("main", { name: "Conversation" }).hover();
  }
  for (let step = 0; step < 4; step += 1) {
    await page.mouse.wheel(0, by / 4);
    await pause(250);
  }
}

export default async function threads({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  // The list says who wrote last.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText(PHOTOS).first().waitFor();
  await page
    .getByText(/^You: Got them, thanks Leo/u)
    .first()
    .scrollIntoViewIfNeeded();
  await caption(page, "Where you wrote last, the row starts with “You:”", 2400);

  // Clara's thread: lanes, and its middle folded.
  await page.getByText(KITCHEN).first().click();
  await page.getByRole("heading", { name: KITCHEN }).waitFor();
  await caption(page, "Seven messages, newest first: Clara's newest is open", 2000);
  await page
    .getByRole("button", { name: /^You\s*→ Clara Nunes/u })
    .first()
    .scrollIntoViewIfNeeded();
  await caption(page, "Yours read “You → Clara” and sit in from the left", 2400);
  const more = page.getByRole("button", { name: /^\d+ more messages$/u });
  await more.scrollIntoViewIfNeeded();
  await caption(page, "The middle of a long thread folds into one line", 2000);
  await more.click();
  await pause(800);
  await caption(page, "It opens in place, each message still folded to one line", 2200);

  // Quoted history: folded in the newest message, opened and closed.
  await scroll(page, phone, -2000);
  const newest = page.locator('iframe[title="Message from Clara Nunes"]').first().contentFrame();
  const show = newest.getByText("Show quoted text");
  await show.waitFor();
  await caption(page, "The quoted thread is folded behind “Show quoted text”", 2200);
  await show.click();
  await pause(900);
  await caption(page, "Open it to read the history, found from the mail's markup alone", 2200);
  await newest.getByText("Hide quoted text").click();
  await pause(700);

  // The inline reply stays whole.
  await page
    .getByRole("button", { name: /^You\s*→ Clara Nunes.*Answers below/u })
    .first()
    .click();
  await pause(900);
  const inline = page.locator('iframe[title="Message from Sam Rivera"]').first().contentFrame();
  await inline.getByText("Oiled, with a natural finish", { exact: false }).scrollIntoViewIfNeeded();
  await caption(page, "An answer written between the quotes is never folded", 2400);

  // The reply box stays at the bottom while the thread scrolls.
  await scroll(page, phone, 1600);
  await caption(
    page,
    phone
      ? "Reply and Done stay above the tab bar as the thread scrolls"
      : "The reply box stays pinned at the bottom as the thread scrolls",
    2400,
  );
  await scroll(page, phone, -1600);
  await pause(600);

  // Reply: the composer opens at the bottom, then is discarded unsent.
  await (phone
    ? page.getByRole("button", { name: "Reply", exact: true }).last().click()
    : page.getByRole("button", { name: /^Reply to Clara Nunes/u }).click());
  const own = page.getByRole("button", { name: /Write my own/u });
  await own.waitFor();
  await own.click();
  const composer = page.getByRole("region", { name: /^Reply to/u });
  await composer.waitFor();
  await caption(page, "The composer opens at the bottom, the thread above it", 2400);
  await composer.getByRole("button", { name: "Discard" }).click();
  await pause(900);
  await caption(page, "Discarded: nothing was sent", 1600);
}
