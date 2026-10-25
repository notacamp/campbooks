import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Hold time for a task (hold-time-for-a-task), in Storybook with demo data
// only: three free slots, Length and Calendar; the task then shows when it is
// held, Release takes it off, and the calendar shows the block.
//   pnpm --dir clients storybook
//   pnpm --dir clients record hold-time --base-url http://localhost:6006
const story = (screen: string, id: string) =>
  `/iframe.html?id=screens-${screen}--${id}&viewMode=story`;

export default async function holdTime({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const freeTime = page.getByRole("group", { name: "Free time" });

  await page.goto(story("tasks", "hold-time-idle"));
  await page.getByText("When will you do it?").waitFor();
  await caption(page, "A task you will do: hold time for it", 2400);
  await page
    .getByRole("button", { name: /^Hold time/u })
    .first()
    .click();
  await freeTime.waitFor();
  await caption(page, "Three free slots in your working hours, before it is due", 2800);
  await caption(page, "Length: 15, 30 or 60 minutes · Calendar: Google or Campbooks", 2800);
  await page.getByRole("button", { name: "60 min" }).click();
  await caption(page, "A longer block finds other slots", 2200);
  await page.getByRole("button", { name: "30 min" }).click();
  await freeTime.getByRole("button").first().click();
  await page
    .getByText(/^Held /u)
    .locator("visible=true")
    .first()
    .waitFor();
  await caption(page, "Held, with Undo: the block is on your calendar", 2800);
  await page.getByRole("button", { name: "Release" }).click();
  await caption(page, "Release takes the block off again", 2400);

  await page.goto(story("tasks", "hold-from-home-idle"));
  await page.getByText("Send the signed lease to Marta").first().waitFor();
  await caption(page, "From Home: J, then H on a task", 2200);
  await page.keyboard.press("j");
  await page.keyboard.press("h");
  await page.getByRole("dialog", { name: "Hold time" }).waitFor();
  await caption(page, "The same choices, in a dialog", 2400);
  await freeTime.getByRole("button").first().click();
  await page
    .getByText(/^Held /u)
    .locator("visible=true")
    .first()
    .waitFor();
  await caption(page, "Held: its row says when", 2600);

  await page.goto(story("calendar", "week-with-held-task-idle"));
  await page.getByRole("group", { name: "View" }).waitFor();
  if (phone) {
    await page.keyboard.press("1");
  }
  await caption(page, "On the calendar: the held block, not a Planned row", 2800);
  await page
    .getByRole("button", { name: /Send the lease/u })
    .first()
    .click();
  await caption(page, "Move or remove the block, here or in Google: the task follows", 3000);
}
