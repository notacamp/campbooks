import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// The inbox fitting the window, in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record inbox-fits-screen --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function inboxFitsScreen({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  if (phone) {
    await page.goto(story("unbroken-text-phone"));
    await page.getByRole("main", { name: "Conversation" }).waitFor();
    await caption(page, "A subject and sender with no spaces wrap on a phone", 2500);
    await page.mouse.wheel(0, 300);
    await caption(page, "Nothing scrolls sideways", 2000);
    return;
  }

  await page.goto(story("unbroken-text-desktop"));
  await page.getByRole("main", { name: "Conversation" }).waitFor();
  await caption(page, "A long subject and sender wrap inside the reader", 2500);

  // A narrow desktop window: the reader used to drop below the list here.
  await page.setViewportSize({ width: 1100, height: 900 });
  await page.goto(story("images-shown"));
  await page.getByRole("main", { name: "Conversation" }).waitFor();
  await caption(page, "1100px wide: the conversation stays beside the list", 2500);

  await page.setViewportSize({ width: 1024, height: 900 });
  await caption(page, "1024px: still side by side, the actions wrap", 2500);

  await page.setViewportSize({ width: 1100, height: 900 });
  await page.goto(story("unbroken-text-narrow-desktop"));
  await page.getByRole("main", { name: "Conversation" }).waitFor();
  await caption(page, "Long text at 1100px: it wraps, the page fits the window", 2500);
  await pause(500);
}
