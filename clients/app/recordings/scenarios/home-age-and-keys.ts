import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Home says how long each item has waited, and the arrow keys open items as a
// click does (campbooks-cloud#276), on the real stack: start it as inbox.ts
// says, with a `decide` model configured (CAMPBOOKS_AI__DECIDE__*), since
// without one no need is raised (ADR 0018). Fictional person and mail only;
// nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record home-age-and-keys \
//     --base-url http://localhost:5181

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Wait until Home lists something (the projection runs a moment after the sync). */
async function homeFilled(page: Page, workspace: string): Promise<void> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    await page.goto(`/w/${workspace}`);
    const heading = page.getByRole("heading", { level: 1 });
    await heading.waitFor();
    if (((await heading.textContent()) ?? "").includes("need you")) {
      return;
    }
    await pause(1000);
  }
}

export default async function homeAgeAndKeys({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await homeFilled(page, workspace);
  await page.getByRole("group", { name: /^Actions for/u }).waitFor();
  await pause(800);

  await caption(page, "Every item says how long it has waited, above its due date", 3200);
  // The full date is the age's tooltip.
  const age = page.locator("button[id^='attention-'] time").first();
  const full = await age.getAttribute("title");
  if (full !== null) {
    await age.hover();
    await caption(page, `Hover it for the full date: ${full}`, 2800);
  }

  if (isPhone(page)) {
    await caption(page, "Tap an item to open it", 1600);
    await page.locator("button[id^='attention-']").nth(1).click();
    await pause(2400);
    return;
  }

  await caption(page, "↓ or J opens the item it lands on, as a click does", 2200);
  await page.keyboard.press("ArrowDown");
  await pause(2200);
  await page.keyboard.press("j");
  await pause(2200);
  await caption(page, "↑ or K goes back, open too", 1600);
  await page.keyboard.press("k");
  await pause(2000);
  await caption(page, "Esc closes it", 1400);
  await page.keyboard.press("Escape");
  await pause(1400);
  await caption(page, "Enter opens it again", 1400);
  await page.keyboard.press("Enter");
  await pause(1800);
  await caption(page, "Done with it (E): the next item opens in its place", 2400);
  await page.keyboard.press("e");
  await pause(2800);
}
