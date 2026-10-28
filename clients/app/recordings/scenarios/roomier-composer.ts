import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// The roomier composer (campbooks-cloud#390), in Storybook with demo data
// only: a reply draft open in its thread, saved in memory. Nothing is sent.
//   pnpm --dir clients storybook
//   pnpm --dir clients record roomier-composer --base-url http://localhost:6006
const STORY = "/iframe.html?id=screens-inbox--reply-draft-open&viewMode=story";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function roomierComposer({ page }: ScenarioContext): Promise<void> {
  const phone = isPhone(page);
  const mod = process.platform === "darwin" ? "Meta" : "Control";

  await page.goto(STORY);
  const composer = page.getByRole("region", { name: /^Reply to/u });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "The draft opens with its formatting shown, not its marks", 2200);

  // Bold a selection: it shows as bold, not as asterisks.
  await body.click();
  await page.keyboard.press(`${mod}+End`);
  await page.keyboard.press("Enter");
  await body.pressSequentially("I will renew for a year and read the lease.", { delay: 25 });
  await page.keyboard.press("Home");
  for (let i = 0; i < 7; i += 1) {
    await page.keyboard.press("ArrowRight");
  }
  for (let i = 0; i < 15; i += 1) {
    await page.keyboard.press("Shift+ArrowRight");
  }
  await page.keyboard.press(`${mod}+b`);
  await caption(page, "Select words and press ⌘B: they show as bold");

  // A link with words, by key.
  await page.keyboard.press("End");
  await page.keyboard.press("ArrowLeft");
  for (let i = 0; i < 9; i += 1) {
    await page.keyboard.press("Shift+ArrowLeft");
  }
  await page.keyboard.press(`${mod}+Shift+k`);
  await composer.getByRole("textbox", { name: "Address" }).fill("https://example.com/lease");
  await caption(page, "⇧⌘K links the words to an address", 1800);
  await composer.getByRole("button", { name: "Add link" }).click();

  // A bullet list.
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await page.keyboard.press(`${mod}+Shift+8`);
  await body.pressSequentially("parking P2-14", { delay: 25 });
  await page.keyboard.press("Enter");
  await body.pressSequentially("rent on the 1st", { delay: 25 });
  await caption(page, "⇧⌘8 starts a bulleted list");
  await composer.getByText("Saved").waitFor();

  if (phone) {
    await composer.getByRole("button", { name: "More formatting" }).click();
    await caption(page, "On a phone the toolbar stays one row: the rest is under More", 2200);
    await page.keyboard.press("Escape");
    await pause(600);
    return;
  }

  // Expand and back.
  await page.keyboard.press(`${mod}+Shift+e`);
  const sheet = page.getByRole("dialog", { name: /^Reply to/u });
  await sheet.getByRole("button", { name: "Shrink" }).waitFor();
  await caption(page, "⇧⌘E opens the same draft as a sheet, cursor and all", 2200);
  await sheet.getByRole("textbox", { name: "Message" }).pressSequentially(" Thank you!", {
    delay: 25,
  });
  await page.keyboard.press("Escape");
  await composer.getByRole("button", { name: "Expand" }).waitFor();
  await pause(600);
  await caption(page, "Esc puts it back where it was", 1800);
}
