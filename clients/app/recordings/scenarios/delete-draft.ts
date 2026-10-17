import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption, signedIn } from "./inbox.ts";

// Delete a draft (campbooks-cloud#238) on the real stack, as the `inbox`
// scenario runs it (see its header for the commands). Fictional mail only;
// nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5191 pnpm --dir clients record delete-draft \
//     --base-url http://localhost:5191

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

const Q4 = "Q4 plan: can you review by Friday?";
const CHIP = /^Draft saved · Reply to/u;

export default async function deleteDraft({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText(Q4).first().click();
  await page.getByRole("heading", { name: Q4 }).waitFor();
  await (phone
    ? page.getByRole("button", { name: "Reply", exact: true }).last().click()
    : page.keyboard.press("r"));

  const composer = page.getByRole("region", { name: /^Reply to/u });
  const body = composer.getByRole("textbox", { name: "Message" });
  await body.pressSequentially("Thanks, I will look at it on Thursday.", { delay: 25 });
  await composer.getByText("Saved").waitFor();
  await caption(page, "A reply, saved as you type");

  await body.press("Escape");
  const chip = page.getByRole("button", { name: CHIP });
  await chip.waitFor();
  await caption(page, "Esc keeps it: the draft waits under the thread, with Delete draft beside it");

  await page.getByRole("button", { name: /^Delete draft: Reply to/u }).click();
  await page.getByText("Draft deleted").waitFor();
  await chip.waitFor({ state: "detached" });
  await caption(page, "Deleted, with Undo in the toast");

  await page.getByRole("button", { name: /^Undo/u }).click();
  await chip.waitFor();
  await caption(page, "Undo brings the draft back");

  await page.getByRole("button", { name: /^Delete draft: Reply to/u }).click();
  await page.getByText("Draft deleted").waitFor();
  await chip.waitFor({ state: "detached" });
  await pause(1200);
  await caption(page, "Deleted again: gone for good once the toast goes", 1800);
}
