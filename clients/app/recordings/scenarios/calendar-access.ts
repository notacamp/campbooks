import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Adding calendar access (campbooks-cloud#373), in Storybook with demo data
// only, at 375px and on the desktop: a Google and a Zoho mailbox that gave
// mail only are both offered it, and when the sign-in is too old the page
// says so and offers Sign in again instead of a dead end.
//   pnpm --dir clients storybook
//   pnpm --dir clients record calendar-access --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-calendar--${id}&viewMode=story`;

export default async function calendarAccess({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("settings-step-up-idle"));
  const access = page.getByRole("button", { name: "Add calendar access" });
  await access.first().waitFor();
  await access.first().scrollIntoViewIfNeeded();
  await caption(page, "Settings → Calendar: Google and Zoho mailboxes without a calendar", 2600);

  await access.first().click();
  const again = page.getByRole("button", { name: "Sign in again" });
  await again.waitFor();
  await again.scrollIntoViewIfNeeded();
  await caption(page, "Sign-in too old: it says so and offers Sign in again", 3000);
}
