import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Calendar roles in Settings → Calendar (campbooks-cloud#399), in Storybook
// with demo data only.
//   pnpm --dir clients storybook
//   pnpm --dir clients record calendar-roles --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-calendar--${id}&viewMode=story`;

export default async function calendarRoles({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const role = (name: string) => page.getByRole("group", { name: `Role of ${name}` });

  await page.goto(story(phone ? "settings-roles-idle-phone" : "settings-roles-idle"));
  await page.getByRole("list", { name: "Mine" }).waitFor();
  await caption(page, "Settings → Calendar: your calendars, the ones you follow, and Off", 2600);

  await page.getByText("Shared with you").first().scrollIntoViewIfNeeded();
  await caption(page, "Marta's calendar is shared with you: Following, shown only", 2400);

  await role("Work").scrollIntoViewIfNeeded();
  await pause(400);
  await role("Work").getByRole("button", { name: "Following" }).click();
  await caption(
    page,
    "Work moves to Following: it no longer makes you busy or takes your tasks",
    2600,
  );

  await page.getByRole("button", { name: "Undo" }).click();
  await caption(page, "Undo puts it back under Mine", 2000);

  const holidays = role("Holidays in Portugal");
  await holidays.scrollIntoViewIfNeeded();
  await pause(400);
  await holidays.getByRole("button", { name: "Following" }).click();
  await caption(page, "Holidays on, as Following: on the calendar, never your time", 2600);
}
