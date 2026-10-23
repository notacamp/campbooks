import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// The Calendar place (campbooks-cloud#42), in Storybook with demo data only:
// the agenda, Week and Month, a Scout event with its email and "How Scout
// knew", Remove with Undo, a task's sheet, the reader's "Scout noticed" strip
// and Settings → Calendar.
//   pnpm --dir clients storybook
//   pnpm --dir clients record calendar --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-calendar--${id}&viewMode=story`;

export default async function calendar({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(story("calendar-idle"));
  await page.getByText("Dentist check-up").first().waitFor();
  await caption(page, "Calendar: Scout's finds from email and your day", 1800);

  // Week (2) and Month (3), by key on a keyboard and by tab on a phone.
  for (const [key, name, label] of [
    ["2", "Week", "Week"],
    ["3", "Month", "Month"],
    ["1", "Agenda", "Back to Agenda"],
  ] as const) {
    if (phone) {
      await page
        .getByRole("group", { name: "View" })
        .getByRole("button", { name: new RegExp(`^${name}`, "u") })
        .click();
    } else {
      await page.keyboard.press(key);
    }
    await caption(page, phone ? label : `${label} (${key})`, 1500);
  }

  // A Scout event: its email, and how Scout knew the date.
  await page.getByText("Dentist check-up").first().click();
  const sheet = page.getByRole("complementary", { name: "Dentist check-up" });
  await sheet.getByText("How Scout knew").waitFor();
  await caption(page, "Its email, and the sentence Scout read the date from", 2800);

  // Remove, with Undo.
  await sheet.getByRole("button", { name: /Remove/u }).click();
  await page.getByText(/Dentist check-up removed/u).waitFor();
  await caption(page, "Remove: gone, with Undo", 1600);
  await page.getByRole("button", { name: /Undo/u }).click();
  await page.getByText("Dentist check-up").first().waitFor();
  await caption(page, "Undo brings it back", 1500);

  // A task due today.
  await page.getByText(/Pay · Costa Obras invoice/u).first().click();
  await page.getByRole("complementary", { name: /Pay · Costa Obras invoice/u }).waitFor();
  await caption(page, "A task: pay, hold time, later or open it", 2200);

  // What Scout noticed, in the reader.
  await page.goto(story("reader-idle"));
  await page.getByRole("region", { name: "Scout noticed" }).waitFor();
  await caption(page, "In the email: what Scout noticed, on your calendar", 2800);

  // Settings → Calendar.
  await page.goto(story("settings-idle"));
  await page.getByRole("heading", { name: "Calendars" }).waitFor();
  await caption(page, "Settings → Calendar", 2400);
  await pause(300);
}
