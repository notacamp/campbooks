import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Composer controls (campbooks-cloud#394), in Storybook with demo data only:
// a reply draft open in its thread, kept in memory. Nothing is sent.
//   pnpm --dir clients storybook
//   pnpm --dir clients record composer-controls --base-url http://localhost:6006
const STORY = "/iframe.html?id=screens-inbox--reply-draft-open&viewMode=story";

export default async function composerControls({ page }: ScenarioContext): Promise<void> {
  await page.goto(STORY);
  const composer = page.getByRole("region", { name: /^Reply to/u });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.waitFor();
  await caption(page, "A reply: its subject is there to edit, and From is a picker", 2600);

  // From.
  const from = composer.getByRole("combobox", { name: "From" });
  await from.selectOption("0192f1a0-0000-7000-8000-0000000000b2");
  await composer.getByText("Sam Rivera, Rivera Lettings").waitFor();
  await caption(page, "From moves the draft to another mailbox, and its signature with it", 2800);

  // The title's menu.
  await composer.getByRole("button", { name: "Change reply type" }).click();
  await page.getByRole("menuitemradio", { name: /Reply all/u }).waitFor();
  await caption(page, "The title switches Reply, Reply all and Forward, keeping the text", 2800);
  await page.keyboard.press("Escape");

  // Scout.
  await composer.getByRole("button", { name: "Ask Scout to rewrite" }).click();
  await page.getByRole("button", { name: "Shorter" }).waitFor();
  await caption(page, "Scout (⌘J): Shorter, Warmer, Fix spelling, or a change in your words", 2800);
  await page.getByRole("button", { name: "Shorter" }).click();
  await page.getByText("Scout is rewriting…").waitFor();
  await caption(page, "The text is read-only while Scout works, and its thinking shows", 1800);
  await page.getByText("Rewritten by Scout").waitFor();
  await caption(page, "It lands as plain text, with Undo", 2400);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await caption(page, "Undo puts the previous text back", 2000);
  await pause(600);
}
