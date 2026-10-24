import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Calendar sync and Scout's calendar proposals (campbooks-cloud#42), in
// Storybook with demo data only, at 375px and on the desktop:
//   pnpm --dir clients storybook
//   pnpm --dir clients record calendar-sync --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

const SAID =
  "Move the dentist to Thursday at 10:00, accept the design review and put the key handover on my Google calendar.";

export default async function calendarSync({ page }: ScenarioContext): Promise<void> {
  // Settings: a mailbox that gave mail only can add its calendar.
  await page.goto(story("screens-calendar--settings-add-access-idle"));
  const access = page.getByRole("button", { name: "Add calendar access" });
  await access.waitFor();
  await access.scrollIntoViewIfNeeded();
  await caption(page, "Settings: a Google mailbox without its calendar", 1800);
  await caption(page, "Add calendar access asks Google once", 2200);

  // An invitation Google would not take a change to.
  await page.goto(story("screens-calendar--invitation-sheet-idle"));
  const sheet = page.getByRole("complementary", { name: "Design review with Inês" });
  await sheet.waitFor();
  await sheet.getByText(/Changed in Google meanwhile; we kept theirs/u).waitFor();
  await caption(page, "Changed in Google meanwhile: we keep theirs and say so", 3000);

  // The reader: add what Scout found to a Google calendar.
  await page.goto(story("screens-calendar--reader-google-idle"));
  const noticed = page.getByRole("region", { name: "Scout noticed" });
  await noticed.waitFor();
  await caption(page, "In the email: what Scout noticed", 1800);
  await noticed.getByText("Key handover at the flat").first().waitFor();
  await caption(page, "Add to my calendar puts it on Google too", 2400);

  // Scout proposes calendar changes; you confirm.
  await page.goto(story("screens-scout--calendar-proposals-live"));
  const marta = page.getByText("Marta Ferreira is waiting for your reply");
  await marta.waitFor();
  await marta.click();
  await page.getByRole("button", { name: /Continue with Scout/u }).click();
  const field = page.getByRole("textbox", { name: "Ask about this" });
  await field.waitFor();
  await caption(page, "Say what you want on your calendar", 1800);
  await field.click();
  await field.pressSequentially(SAID, { delay: 30 });
  await page.keyboard.press("Enter");
  const move = page.getByRole("region", { name: /^Move this event to/u });
  await move.waitFor({ timeout: 20_000 });
  await caption(page, "Scout proposes: each card names the event from its record", 2800);
  await move.getByRole("button", { name: "Confirm" }).click();
  await move.getByText("Done.").waitFor();
  await caption(page, "Confirm moves the dentist, with Undo", 2200);
  await move.getByRole("button", { name: "Undo" }).click();
  await move.getByText("Undone.").waitFor();
  await caption(page, "Undo puts the old time back", 2200);

  const accept = page.getByRole("region", { name: /^Reply: / });
  await accept.scrollIntoViewIfNeeded();
  await accept.getByRole("button", { name: "Confirm" }).click();
  await accept.getByText("Done.").waitFor();
  await caption(page, "The invitation is answered", 1800);

  const push = page.getByRole("region", { name: /^Add to / });
  await push.scrollIntoViewIfNeeded();
  await caption(page, "The calendar is named from your calendars", 2400);
  await push.getByRole("button", { name: "Confirm" }).click();
  await push.getByText("Done.").waitFor();
  await caption(page, "Added to your Google calendar; Undo takes it off", 2200);
  await pause(600);
}
