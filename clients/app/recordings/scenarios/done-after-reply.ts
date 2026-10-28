import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// "You replied. Done?" (campbooks-cloud#396), in Storybook with demo data
// only: after the member replies in the thread a Do task came from, Home,
// the Tasks row and the task's detail ask whether it is done.
//   pnpm --dir clients storybook
//   pnpm --dir clients record done-after-reply --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-tasks--${id}&viewMode=story`;

export default async function doneAfterReply({ page }: ScenarioContext): Promise<void> {
  const question = "You answered Marta Ferreira’s email. Is this task done?";

  await page.goto(story("replied-on-home-idle"));
  await page.getByText(question).waitFor();
  await caption(page, "You replied to Marta, so Home asks if the task is done", 2600);
  await page.keyboard.press("2");
  await page.getByText("Okay, still open · Send the signed lease").first().waitFor();
  await caption(page, "Not yet: the task stays open, Undo puts the question back", 2400);

  await page.goto(story("replied-on-home-idle"));
  await page.getByText(question).waitFor();
  await page.keyboard.press("1");
  await page.getByText("Done · Send the signed lease").first().waitFor();
  await caption(page, "Done closes it, with Undo", 2200);

  await page.goto(story("replied-row-idle"));
  await page.getByText("You replied · Done?").first().waitFor();
  await caption(page, "The Tasks list shows it on the row", 2200);

  await page.goto(story("replied-detail-idle"));
  await page.getByText("Is this task done?").first().waitFor();
  await caption(page, "The task asks too, and lists your reply under where it came from", 2600);
}
