import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// A full inbox scrolling inside its panes, in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record inbox-panes-scroll --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function inboxPanesScroll({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  await page.goto(story("many-conversations-desktop"));
  await page.getByRole("main", { name: "Conversation" }).waitFor();

  if (phone) {
    await caption(page, "Phones are unchanged: the conversation is its own screen", 2500);
    await page
      .getByRole("button", { name: /Back to/ })
      .first()
      .click();
    await page.getByRole("region", { name: "Conversations" }).waitFor();
    await page.mouse.wheel(0, 1200);
    await caption(page, "and the list scrolls as a page", 2500);
    return;
  }

  await caption(page, "Forty conversations in a 1440×900 window", 2500);
  const list = page.getByRole("region", { name: "Conversations" });
  await list.hover();
  for (let step = 0; step < 6; step += 1) {
    await page.mouse.wheel(0, 500);
    await pause(250);
  }
  await caption(
    page,
    "The list scrolls on its own; its header, the sidebar and the account stay put",
    3000,
  );

  await page.getByRole("main", { name: "Conversation" }).hover();
  await page.mouse.wheel(0, 400);
  await caption(page, "The conversation scrolls on its own too", 2500);

  await page.setViewportSize({ width: 1100, height: 800 });
  await caption(page, "1100px wide: the page still fits the window", 2500);
  await pause(500);
}
