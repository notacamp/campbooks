import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// When Scout could not read a message (campbooks-cloud#375), the thread says
// so instead of leaving "Where things stand" blank. Storybook, demo data only.
//   pnpm --dir clients storybook
//   pnpm --dir clients record thread-read-failed --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function threadReadFailed({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("thread-read-failed-recording"));
  const line = page.getByText("Scout couldn’t read this one.").first();
  await line.waitFor();
  await line.scrollIntoViewIfNeeded();
  await caption(page, "Scout's read of this email failed twice. The thread says so", 2800);
  await caption(page, "The next message in the thread gets a read of its own", 2600);
  await pause(800);
}
