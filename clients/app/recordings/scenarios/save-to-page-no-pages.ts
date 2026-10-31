import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A workspace with no pages yet is offered "Save to page" (campbooks-cloud#403),
// in Storybook with demo data only: pages.list now says whether mail and
// Scout in Pages is on, so the reader no longer needs a page to ask about.
//   pnpm --dir clients storybook
//   pnpm --dir clients record save-to-page-no-pages --base-url http://localhost:6006
const story = (id: string) =>
  `/iframe.html?id=screens-pages-from-mail-and-scout--${id}&viewMode=story`;

export default async function saveToPageNoPages({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("save-to-page-with-no-pages-idle"));
  await page.getByTitle("Message from Maya Okafor").first().waitFor();
  await caption(page, "A workspace with no pages yet", 2200);
  // On a phone Save to page sits in More actions; on the desktop it may be in
  // the toolbar or its own "More" menu.
  const save = page.getByRole("button", { name: /Save to page/u }).first();
  if (!(await save.isVisible())) {
    await page.getByRole("button", { name: /^More/u }).first().click();
    await page.waitForTimeout(300);
  }
  await page
    .getByRole("button", { name: /Save to page/u })
    .first()
    .click();
  await page.getByRole("button", { name: "New page" }).first().waitFor();
  await caption(page, "Save to page is offered", 2200);
  await page.getByRole("button", { name: "New page" }).first().click();
  await page.waitForTimeout(300);
  await caption(page, "New page makes the workspace's first page", 2600);
}
