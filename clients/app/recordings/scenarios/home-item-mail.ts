import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// An opened Home item shows the email (campbooks-cloud#203) on the real
// stack: start it as inbox.ts says, with a `decide` model configured
// (CAMPBOOKS_AI__DECIDE__*), since without one no need is raised (ADR
// 0018). Fictional person and mail only; nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record home-item-mail \
//     --base-url http://localhost:5181

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

export default async function homeItemMail({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  await homeFilled(page, workspace);
  await page.getByRole("group", { name: /^Actions for/u }).waitFor();

  // Open a reply: the email shows under it, a few lines deep.
  await page.getByText("Maya Okafor is waiting for your reply").first().click();
  await page.getByRole("button", { name: "Show the whole email" }).first().waitFor();
  await caption(page, "Open an item to read the email before you decide", 2600);
  await page.getByRole("button", { name: "Show the whole email" }).first().click();
  await caption(page, "Show the whole email reads it in place", 2200);
  await page.getByRole("button", { name: "Show less" }).first().click();
  await pause(800);

  // A warning: the email shows, its links lead nowhere.
  await page.getByText("This may be phishing. Don’t use its links.").first().click();
  await pause(600);
  await caption(page, "In mail that may be phishing, links lead nowhere", 2600);
}
