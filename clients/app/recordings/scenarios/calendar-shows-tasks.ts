import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Tasks on the Calendar (calendar-shows-tasks), in Storybook with demo data
// only: each open task on the day it is due and on the day it is planned.
//   pnpm --dir clients storybook
//   pnpm --dir clients record calendar-shows-tasks --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-calendar--${id}&viewMode=story`;

export default async function calendarShowsTasks({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const lease = page.getByRole("button", { name: /Send the lease/u });

  if (phone) {
    await page.goto(story("agenda-with-tasks-idle"));
    await page.getByRole("group", { name: "View" }).waitFor();
    await caption(page, "Your tasks are on the agenda now", 2400);
    await caption(page, "Overdue ones and slipped plans stand on today", 2600);
    await lease.first().scrollIntoViewIfNeeded();
    await caption(page, "Send the lease: planned Wednesday, due Thursday", 2600);
    await lease.first().click();
    await caption(page, "Its sheet: when you planned it, when it is due", 2600);
    await page.getByRole("button", { name: /^Done/u }).click();
    await caption(page, "Done, with Undo, and it leaves the calendar", 2600);
    return;
  }

  await page.goto(story("week-with-tasks-idle"));
  await page.getByRole("group", { name: "View" }).waitFor();
  await caption(page, "The week now has a Tasks lane", 2400);
  await caption(page, "Dashed: the day it is due · Solid: the day you planned it", 3000);
  await lease.first().click();
  await caption(page, "Send the lease: planned Wednesday, due Thursday", 2600);
  await page.getByRole("button", { name: /^Done/u }).click();
  await caption(page, "Done, with Undo: both of its days clear", 2600);
  await page.keyboard.press("3");
  await caption(page, "Month shows tasks too, after each day's events (3)", 2600);
  await page.keyboard.press("1");
  await caption(page, "And the agenda (1)", 2200);
}
