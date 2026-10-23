import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Reply beside Reply all and Forward, the actions on one row
// (campbooks-cloud#307), in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record reply-in-toolbar --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function replyInToolbar({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  await page.goto(story("thread"));
  await page.getByRole("main", { name: "Conversation" }).waitFor();
  await pause(800);

  if (phone) {
    await caption(page, "Phones: Reply and Done stay above the tab bar", 2400);
    await caption(page, "The header is one row; the rest is under …", 2200);
    await page.getByRole("button", { name: "More actions" }).click();
    await caption(page, "Delete, Report spam, Pin, Mark unread", 2400);
    await page.keyboard.press("Escape");
    await pause(600);
    return;
  }

  await page.getByRole("button", { name: "Reply", exact: true }).hover();
  await caption(page, "One row: Done, then Reply, Reply all, Forward and Move", 2600);
  await caption(page, "Every action after Done is an icon; its tooltip names it and its key", 2600);
  await page.mouse.move(900, 600);
  await page.mouse.wheel(0, 700);
  await caption(page, "Nothing is pinned at the bottom: the thread gets the room", 2400);
  await page.mouse.wheel(0, -700);
  await pause(500);

  await page.keyboard.press("r");
  await page.getByRole("dialog").waitFor();
  await caption(page, "R (or the button) opens Scout's reply dialog, as before", 2400);
  await page.keyboard.press("Escape");
  await pause(600);

  // A narrower window: what has no room folds into More, still one row.
  await page.setViewportSize({ width: 1100, height: 900 });
  await caption(page, "1100px: what doesn't fit folds into …, still one row", 2400);
  await page.getByRole("button", { name: "More actions" }).click();
  await caption(page, "Each with its name and key; ↑/↓ move, Esc closes", 2400);
  await page.keyboard.press("ArrowDown");
  await pause(500);
  await page.keyboard.press("ArrowDown");
  await pause(700);
  await page.keyboard.press("Escape");
  await pause(600);
}
