import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Scout proposes task changes, you confirm (campbooks-cloud#43), in
// Storybook with demo data only, at 375px and on the desktop:
//   pnpm --dir clients storybook
//   pnpm --dir clients record scout-proposes-task-changes --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

const SAID =
  "I paid the electricity bill, I will send Marta the lease on Friday, and Rui can take the proposal draft.";

export default async function scoutProposesTaskChanges({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("screens-scout--task-proposals-live"));
  const marta = page.getByText("Marta Ferreira is waiting for your reply");
  await marta.waitFor();
  await caption(page, "Home: Marta is waiting for a reply", 1400);
  await marta.click();
  await page.getByRole("button", { name: /Continue with Scout/u }).click();
  const field = page.getByRole("textbox", { name: "Ask about this" });
  await field.waitFor();
  await caption(page, "Say what happened to your tasks", 1800);
  await field.click();
  await field.pressSequentially(SAID, { delay: 30 });
  await page.keyboard.press("Enter");
  const bill = page.getByRole("region", { name: "Mark paid" });
  await bill.waitFor({ timeout: 20_000 });
  await caption(page, "Scout proposes: each card names the task from its record", 2800);
  await bill.getByRole("button", { name: "Confirm" }).click();
  await bill.getByText("Done.").waitFor();
  await caption(page, "Confirm sends your own request, with Undo", 2200);
  await bill.getByRole("button", { name: "Undo" }).click();
  await bill.getByText("Undone.").waitFor();
  await caption(page, "Undo puts it back, only if nothing changed since", 2200);

  const plan = page.getByRole("region", { name: /^Plan it for/u });
  await plan.scrollIntoViewIfNeeded();
  await plan.getByRole("button", { name: "Confirm" }).click();
  await plan.getByText("Done.").waitFor();
  await caption(page, "Planned for Friday", 1800);

  const hand = page.getByRole("region", { name: "Hand to Rui Matos" });
  await hand.scrollIntoViewIfNeeded();
  await caption(page, "The teammate is named from your workspace's members", 2600);
  await hand.getByRole("button", { name: "Confirm" }).click();
  await hand.getByText("Done.").waitFor();
  await caption(page, "Handed to Rui; you can take it back", 2200);
  await pause(800);
}
