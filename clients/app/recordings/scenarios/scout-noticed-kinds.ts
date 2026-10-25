import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout noticed one kind of a sender's mail (campbooks-cloud#356), in
// Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record scout-noticed-kinds --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-scout--${id}&viewMode=story`;

export default async function scoutNoticedKinds({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("scout-noticed-kinds-live"));
  const note = page.getByRole("region", { name: "Scout noticed" });
  await note.waitFor();
  await note.scrollIntoViewIfNeeded();
  await caption(page, "Scout noticed the shop's weekly deals, never its shipping notices", 3500);
  await caption(page, "It quotes a real subject of that kind", 2500);

  // Not now (Shift+L) shows the next kind.
  await page.keyboard.press("Shift+L");
  await note.getByText(/sale starts now/u).waitFor();
  await caption(page, "The shop's promotions: words Scout's model found in their subjects", 3500);

  // See the 12 emails (Shift+S): only that kind.
  await page.keyboard.press("Shift+S");
  await page.getByText("Our scarves sale starts now").first().waitFor();
  await caption(page, "See the emails: only the promotions", 2500);
  // The end of the query: the words that narrow it.
  const query = page.locator('input[value^="from:"]').first();
  await query.focus();
  await page.keyboard.press("End");
  await caption(page, 'The search adds subject:"sale starts now"', 3000);
  await pause(500);
}
