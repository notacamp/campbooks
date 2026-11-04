import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout's explanation with points that come from several messages
// (campbooks-cloud#422), in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record explanation-chips --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=components-explanation--${id}&viewMode=story`;

export default async function explanationChips({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(story("many-sources"));
  const card = page.getByRole("region", { name: "Scout's explanation" });
  await card.waitFor();

  if (phone) {
    await caption(page, "On a phone: In short first", 2000);
    await card.getByRole("button", { name: "Who wants what, agreed, open" }).click();
    await caption(page, "Each point's messages sit under it and wrap", 2500);
    await page.mouse.wheel(0, 300);
    await caption(page, "A long address shortens; nothing scrolls sideways", 2500);
    return;
  }

  await caption(page, "As wide as in the reader: one column, the text reads across", 3000);
  await card.getByRole("button", { name: "You · 23 Oct" }).hover();
  await caption(page, "Your own messages say You; the chips wrap under the point", 2500);

  await page.goto(story("many-sources-wide"));
  await page.getByRole("region", { name: "Scout's explanation" }).waitFor();
  await caption(page, "A wide card: two columns, chips still under each point", 3000);
  await pause(500);
}
