import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Teach once (campbooks-cloud#125) on the real stack, as `inbox.ts` runs it,
// with the fake Google's mailbox holding 24 more Design Notes digests:
//
//   pnpm --dir clients fake-google --port 9071 --lookalikes 24
//   (the server, worker and Vite as in inbox.ts)
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record teach-once --base-url http://localhost:5181
//
// Moving one digest to Newsletters offers to move every one like it, and
// new ones too; accepting makes a rule whose run moves them. Fictional mail
// and person only.

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function teachOnce({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await page.getByText("This week: calmer dashboards").first().waitFor();
  await caption(page, "Primary holds a newsletter, issue after issue", 2000);
  await page.getByText("This week: calmer dashboards").first().click();
  await page.getByRole("heading", { name: "This week: calmer dashboards" }).waitFor();

  await caption(page, "Move this one to Newsletters (M, then 4)", 1800);
  if (phone) {
    await page.getByRole("button", { name: /^Move/u }).first().click();
    await page
      .getByRole("button", { name: /Newsletters/u })
      .last()
      .click();
  } else {
    await page.keyboard.press("m");
    await pause(500);
    await page.keyboard.press("4");
  }

  const offer = page.getByRole("dialog", { name: /like this to Newsletters/u });
  await offer.waitFor();
  await caption(
    page,
    "Campbooks offers to do the same to every one like it, and to new ones",
    2600,
  );
  await caption(page, "It says which mail, how many, and shows a few", 2200);

  await caption(page, "Accept (1)", 1200);
  await (phone
    ? offer.getByRole("button", { name: /Move all/u }).click()
    : page.keyboard.press("1"));
  await page
    .getByRole("dialog", { name: /New ones will go there too/u })
    .waitFor({ timeout: 60_000 });
  await caption(page, "A rule moved them in the background, and will move new ones", 2600);

  await page.goto(`/w/${workspace}/inbox/newsletters`);
  await page.getByRole("heading", { level: 1, name: "Newsletters" }).waitFor();
  await page
    .getByText(/Campbooks moved this/u)
    .first()
    .waitFor();
  await caption(
    page,
    "Each one says so: Campbooks moved this, like the others from the digest",
    2600,
  );
  await page
    .getByText(/This week: typography/u)
    .first()
    .click();
  await page.getByRole("button", { name: "Stop doing this" }).waitFor();
  await caption(page, "And the rule can be stopped from any of them", 2400);
  await pause(800);
}
