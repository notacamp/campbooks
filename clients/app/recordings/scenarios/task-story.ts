import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A task says why it's here and what happened to it (campbooks-cloud#397),
// in Storybook with demo data only: who made it and when, the email it came
// from, then its history in plain sentences.
//   pnpm --dir clients storybook
//   pnpm --dir clients record task-story --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tasks--${id}&viewMode=story`;

export default async function taskStory({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("why-and-history-idle"));
  await page.getByText("Scout made it").first().waitFor();
  await caption(page, "Why it's here: Scout made it, from this email", 2600);
  const history = page.getByRole("heading", { name: "History" });
  await history.scrollIntoViewIfNeeded();
  await page.getByText("You marked it done").first().waitFor();
  await caption(page, "Its history: every change, who made it, and when", 3000);
  await page
    .getByText(/^The calendar moved the held time/u)
    .first()
    .scrollIntoViewIfNeeded();
  await caption(page, "Scout and the calendar are named too", 2400);
}
