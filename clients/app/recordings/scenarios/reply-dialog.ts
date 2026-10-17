import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Reply proposes Scout's draft (campbooks-cloud#168) on the real stack: start
// it as inbox.ts says, with a `decide` model configured (so needs are raised,
// ADR 0018) and the fake `complete` model for the drafts, which answers with
// canned, fictional text and keeps nothing:
//
//   pnpm --dir clients fake-complete --port 9060
//   CAMPBOOKS_AI__COMPLETE__BASE_URL=http://127.0.0.1:9060/v1 \
//   CAMPBOOKS_AI__COMPLETE__MODEL=fake-complete  (plus inbox.ts's settings)
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record reply-dialog \
//     --base-url http://localhost:5181
//
// Fictional person and mail only. The send goes to the fake mailbox only;
// the scenario undoes it, so nothing leaves.

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Wait until Home lists something (the projection runs a moment after the sync). */
async function homeFilled(page: Page, workspace: string): Promise<void> {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    await page.goto(`/w/${workspace}`);
    const heading = page.getByRole("heading", { level: 1 });
    await heading.waitFor();
    if (((await heading.textContent()) ?? "").includes("need you")) {
      return;
    }
    await pause(1000);
  }
}

/** Reply on the selected item: the button on a phone, R on a desktop. */
async function openReply(page: Page, phone: boolean): Promise<void> {
  await (phone
    ? page
        .getByRole("button", { name: /^Reply/u })
        .first()
        .click()
    : page.keyboard.press("r"));
}

export default async function replyDialog({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeFilled(page, workspace);
  await page.getByRole("group", { name: /^Actions for/u }).waitFor();

  // Select the first item that waits for a reply.
  await page
    .getByText(/is waiting for your reply/u)
    .first()
    .click();
  await caption(page, "Reply now asks Scout first", 1600);
  await openReply(page, phone);

  const dialog = page.getByRole("dialog", { name: /^Reply to/u });
  await dialog.waitFor();
  await caption(page, "Scout reads the conversation and offers likely replies", 2200);
  await dialog.getByRole("button", { name: /Say yes/u }).waitFor();
  await caption(page, "Each one has its key. Esc closes with nothing changed", 2000);
  await page.keyboard.press("Escape");
  await pause(900);

  // Open it again and pick the first: only now does Scout write the draft.
  await openReply(page, phone);
  await dialog.waitFor();
  await dialog.getByRole("button", { name: /Say yes/u }).waitFor();
  await caption(page, "Pick one: only then does Scout write the draft");
  await (phone
    ? dialog.getByRole("button", { name: /Say yes/u }).click()
    : page.keyboard.press("1"));
  const text = dialog.getByRole("textbox", { name: "Your reply" });
  await text.waitFor();
  await caption(page, "The draft is yours to edit, signed with your name", 2400);

  await dialog.getByRole("button", { name: /Shorter/u }).click();
  await caption(page, "Shorter, or ask for a change in plain words", 2000);
  await dialog.getByRole("textbox", { name: "Ask for a change" }).fill("warmer");
  await dialog.getByRole("button", { name: /^Change/u }).click();
  await pause(1600);

  await caption(page, "Send: it goes in a few seconds, with Undo until then", 1800);
  await dialog.getByRole("button", { name: /^Send/u }).click();
  await pause(1400);
  await caption(page, "The item folds away. Undo brings the draft back", 1800);
  await page.getByRole("button", { name: /^Undo/u }).first().click();
  await dialog.waitFor();
  await pause(1800);
  await caption(page, "Open in composer moves the draft to the full composer", 1800);
  await dialog.getByRole("button", { name: "Open in composer" }).click();
  await pause(2400);
}
