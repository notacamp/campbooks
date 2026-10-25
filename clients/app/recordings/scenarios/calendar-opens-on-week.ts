import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// The Calendar's opening view (calendar-opens-on-week), in Storybook with
// demo data only: Week on a wide screen, Agenda on a phone, the other one a
// key or a tap away.
//   pnpm --dir clients storybook
//   pnpm --dir clients record calendar-opens-on-week --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-calendar--${id}&viewMode=story`;

export default async function calendarOpensOnWeek({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  const views = page.getByRole("group", { name: "View" });

  await page.goto(story("calendar-idle"));
  await views.waitFor();
  if (phone) {
    await caption(page, "On a phone the Calendar opens on the agenda", 2400);
    await views.getByRole("button", { name: /^Week/u }).click();
    await caption(page, "The week is one tap away", 2000);
    await views.getByRole("button", { name: /^Agenda/u }).click();
    await caption(page, "And back to the agenda", 1500);
  } else {
    await caption(page, "The Calendar now opens on the week", 2400);
    await page.keyboard.press("1");
    await caption(page, "The agenda is one key away (1)", 2000);
    await page.keyboard.press("2");
    await caption(page, "Back to the week (2)", 1500);
  }
}
