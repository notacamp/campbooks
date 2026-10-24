import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// UI polish (campbooks-cloud#347), in Storybook with demo data only: the
// new-task form in the right pane, the date and time pickers, the compact
// calendar views, and Home's spacing with a readable Scout noticed.
//   pnpm --dir clients storybook
//   pnpm --dir clients record ui-polish --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

export default async function uiPolish({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  // Tasks: the form takes the right pane (the whole screen on a phone).
  await page.goto(story("screens-tasks--list-idle"));
  await page.getByRole("button", { name: /New task/u }).waitFor();
  await caption(page, "Tasks: New task (N T) opens a form beside the list", 2000);
  await page.getByRole("button", { name: /New task/u }).click();
  await page.getByPlaceholder("What needs doing?").waitFor();
  await page.keyboard.type("Renew the parking permit", { delay: 40 });
  await caption(page, "Shaped like a task's page: title, Do or Pay, when it is due", 2000);
  await page.getByRole("button", { name: "Pick a date", exact: true }).click();
  await caption(page, "Pick a date: a month grid, arrows move, Enter picks", 2000);
  await page.keyboard.press("Tab");
  await page
    .getByRole("button", { name: /October 30|30 October/u })
    .first()
    .click();
  await pause(500);
  await page.getByPlaceholder("What needs doing?").focus();
  await page.keyboard.press("Enter");
  await caption(page, "Added with Undo; the form stays open for the next one", 2400);
  await page.keyboard.press("Escape");
  await pause(800);

  // Calendar: the smaller view switch, and the pickers in the event dialog.
  await page.goto(
    story(phone ? "screens-calendar--agenda-phone" : "screens-calendar--agenda-desktop"),
  );
  await page.getByRole("group", { name: /View/u }).first().waitFor();
  await caption(page, "Calendar: Agenda, Week and Month take less room", 2200);
  await page.keyboard.press("n");
  await page.keyboard.press("e");
  const starts = page.getByRole("combobox", { name: "Starts" });
  await starts.waitFor();
  await starts.click();
  await caption(page, "Times: a list every 15 minutes, in your language’s format", 2000);
  await page.keyboard.type("930", { delay: 120 });
  await page.keyboard.press("Enter");
  await caption(page, "…or type it: 930, 9:30, 21h15, 9pm", 2000);
  await page
    .getByRole("dialog")
    .getByRole("button", { name: /Oct 27/u })
    .first()
    .click();
  await caption(page, "Dates: the grid opens over the dialog, never cut off", 2000);
  await page.keyboard.press("ArrowRight");
  await pause(300);
  await page.keyboard.press("ArrowRight");
  await pause(300);
  await page.keyboard.press("Enter");
  await pause(1200);
  await page.keyboard.press("Escape");
  await pause(500);

  // Home: room between the groups, and Scout noticed in the body font.
  await page.goto(story("screens-scout--ask-from-scout-noticed-live"));
  const noticed = page.getByRole("region", { name: "Scout noticed" });
  await noticed.waitFor();
  await noticed.scrollIntoViewIfNeeded();
  await caption(page, "Home: space between groups; Scout noticed reads in the body font", 2600);
  await caption(page, "See the 34 emails (⇧S) opens them in search", 1800);
  await page.keyboard.press("Shift+S");
  await pause(2400);
}
