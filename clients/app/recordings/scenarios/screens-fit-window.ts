import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Every screen fitting the window, in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record screens-fit-window --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

export default async function screensFitWindow({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  await page.goto(story("screens-home--busy-desktop"));
  await page.getByRole("heading", { level: 1, name: "40 things need you" }).waitFor();

  if (phone) {
    await caption(page, "Phones are unchanged: Home scrolls as a page", 2000);
    await page.mouse.wheel(0, 1500);
    await caption(page, "with the tab bar kept at the bottom", 2500);
    return;
  }

  await caption(page, "Forty things need you in a 1440×900 window", 2500);
  await page.getByRole("main").hover();
  for (let step = 0; step < 6; step += 1) {
    await page.mouse.wheel(0, 500);
    await pause(250);
  }
  await caption(page, "Home scrolls on its own; the sidebar and the account stay put", 3000);

  await page.goto(story("screens-inbox--many-conversations-desktop"));
  await page.getByRole("main", { name: "Conversation" }).waitFor();
  await page.getByRole("region", { name: "Conversations" }).hover();
  for (let step = 0; step < 4; step += 1) {
    await page.mouse.wheel(0, 500);
    await pause(250);
  }
  await caption(page, "The inbox list scrolls the same way", 2500);

  await page.emulateMedia({ colorScheme: "dark" });
  await page.evaluate(`document.documentElement.dataset.theme = "dark"`);
  await caption(page, "And in dark", 2500);
  await pause(500);
}
