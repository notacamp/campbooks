import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout proposes, you confirm (campbooks-cloud#305, phase 2), in Storybook
// with demo data only, at 375px and on the desktop:
//   pnpm --dir clients storybook
//   pnpm --dir clients record scout-proposes-actions --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

export default async function scoutProposesActions({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("screens-scout--proposals-live"));
  const marta = page.getByText("Marta Ferreira is waiting for your reply");
  await marta.waitFor();
  await caption(page, "Home: Marta is waiting for a reply", 1400);
  await marta.click();
  await page.getByRole("button", { name: /Continue with Scout/u }).click();
  const field = page.getByRole("textbox", { name: "Ask about this" });
  await field.waitFor();
  await caption(page, "Scout asks before it changes anything", 2000);
  await field.click();
  await field.pressSequentially("I answered Marta by phone. Can you get this out of my inbox?", {
    delay: 35,
  });
  await page.keyboard.press("Enter");
  const archive = page.getByRole("region", { name: "Archive" });
  await archive.waitFor({ timeout: 20_000 });
  await caption(page, "Scout proposes: each card names exactly what and where", 2600);
  await archive.getByRole("button", { name: "Confirm" }).click();
  await archive.getByText("Done.").waitFor();
  await caption(page, "Confirm sends your own request, with Undo", 2200);
  await archive.getByRole("button", { name: "Undo" }).click();
  await archive.getByText("Undone.").waitFor();
  await caption(page, "Undo puts it back", 1800);

  const rule = page.getByRole("region", { name: "A rule to teach Scout" });
  await rule.scrollIntoViewIfNeeded();
  await caption(page, "A rule card: Scout's sentence, quoted", 2000);
  await rule.getByRole("button", { name: "Review the rule" }).click();
  const dialog = page.getByRole("dialog", { name: "Teach Scout something" });
  await dialog.getByRole("button", { name: /Save rule/u }).waitFor({ timeout: 15_000 });
  await caption(page, "Teach Scout reads it; nothing is saved until you press Save rule", 2600);
  await dialog.getByRole("button", { name: /Save rule/u }).click();
  await rule.getByText("Rule saved.").waitFor();
  await caption(page, "Saved by your own yes; Scout reads what you did next time", 2400);
  await pause(800);
}
