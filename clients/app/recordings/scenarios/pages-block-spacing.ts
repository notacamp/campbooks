import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Pages block spacing (campbooks-cloud#398), in Storybook with demo data only:
// a page with every block, scrolled top to bottom, and a toggle closed and
// opened again; on a phone the same page, nothing scrolling sideways.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages-block-spacing --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-pages--${id}&viewMode=story`;

export default async function pagesBlockSpacing({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  await page.goto(story(phone ? "every-block-phone-idle" : "every-block-idle"));
  const editor = page.getByRole("textbox", { name: "Page text" });
  await editor.getByText("Notes from the kickoff go below.").waitFor();
  await caption(page, "Headings have room above them and sit close to their text", 3000);

  const box = await editor.boundingBox();
  if (box !== null) {
    await page.mouse.move(box.x + box.width / 2, box.y + 40);
  }
  const step = phone ? 320 : 360;
  await page.mouse.wheel(0, step);
  await caption(page, "Lists have their markers; to-dos sit as close as a list", 3000);
  await page.mouse.wheel(0, step);
  await caption(page, "Callouts, quotes and cards keep a gap between them", 3000);
  await page.mouse.wheel(0, step);
  await pause(800);

  const toggle = page.getByRole("button", { name: "Hide", exact: true }).first();
  await toggle.scrollIntoViewIfNeeded();
  await caption(page, "A toggle shows its first line…", 2000);
  await toggle.click();
  await caption(page, "…and closed, hides what is inside", 2500);
  await page.getByRole("button", { name: "Show", exact: true }).first().click();
  await pause(800);

  await page.mouse.wheel(0, step * 2);
  await caption(
    page,
    phone ? "Same rhythm on a phone, nothing scrolls sideways" : "Code and the last line",
    2500,
  );
  await pause(500);
}
