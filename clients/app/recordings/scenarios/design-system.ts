import { pause, type ScenarioContext } from "../context.ts";

// Walks the design system's pattern stories in Storybook (demo data only):
//   pnpm --dir clients storybook
//   pnpm --dir clients record design-system --base-url http://localhost:6006
const story = (id: string, globals = "") =>
  `/iframe.html?id=${id}&viewMode=story${globals === "" ? "" : `&globals=${globals}`}`;

export default async function designSystem({ page }: ScenarioContext): Promise<void> {
  // Home: the "needs you" queue, driven from the keyboard.
  await page.goto(story("patterns-home--business"));
  await page.getByRole("heading", { level: 1 }).waitFor();
  await pause(1500);
  await page.keyboard.press("j");
  await pause(600);
  await page.keyboard.press("Enter");
  await pause(1200);
  await page.keyboard.press("e");
  await pause(1500);
  await page.keyboard.press("z");
  await pause(1500);
  await page.keyboard.press("?");
  await pause(2000);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+k");
  await page.keyboard.type("inb", { delay: 120 });
  await pause(1500);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+Shift+w");
  await pause(1500);

  // Inbox: streams, the reading view and the assistant's note.
  await page.goto(story("patterns-inbox--primary"));
  await page.getByRole("heading", { level: 2 }).first().waitFor();
  await pause(1200);
  await page.keyboard.press("j");
  await pause(1200);
  await page.keyboard.press("k");
  await page.getByRole("button", { name: "Why?" }).click();
  await pause(1500);

  // Phone home (dark): a swipe action, then Undo.
  await page.goto(story("patterns-phone-home--dark", "theme:dark"));
  await page.getByRole("heading", { level: 1 }).waitFor();
  await pause(1200);
  await page.getByRole("button", { name: "Done" }).click();
  await pause(1200);
  await page.getByRole("button", { name: "Undo" }).click();
  await pause(1200);

  // States and themes boards.
  await page.goto(story("patterns-states--light"));
  await pause(1500);
  await page.mouse.wheel(0, 900);
  await pause(1500);
  await page.goto(story("patterns-themes--all-themes"));
  await pause(2000);
  await page.mouse.wheel(0, 900);
  await pause(1500);
}
