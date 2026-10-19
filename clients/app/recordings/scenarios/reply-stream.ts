import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Scout's reply streams (campbooks-cloud#288): its thinking shows as it
// arrives, then the reply types into the draft. Start the stack as
// reply-dialog.ts says, with the fake `complete` model, which streams a few
// lines of fictional thinking and then the reply word by word:
//
//   pnpm --dir clients fake-complete --port 9060
//   CAMPBOOKS_AI__COMPLETE__BASE_URL=http://127.0.0.1:9060/v1 \
//   CAMPBOOKS_AI__COMPLETE__MODEL=fake-complete  (plus inbox.ts's settings)
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record reply-stream \
//     --base-url http://localhost:5181
//
// Fictional person and mail only. Nothing is sent: the dialog closes with
// Esc at the end.

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

export default async function replyStream({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeFilled(page, workspace);
  await page.getByRole("group", { name: /^Actions for/u }).waitFor();

  await page
    .getByText(/is waiting for your reply/u)
    .first()
    .click();
  await openReply(page, phone);
  const dialog = page.getByRole("dialog", { name: /^Reply to/u });
  await dialog.waitFor();
  await dialog.getByRole("button", { name: /Say yes/u }).waitFor();
  await caption(page, "Pick a likely reply", 1400);
  await (phone
    ? dialog.getByRole("button", { name: /Say yes/u }).click()
    : page.keyboard.press("1"));

  // The thinking arrives first, then the words type into the draft.
  await dialog.getByText("Scout is thinking…").waitFor();
  await caption(page, "Scout's thinking shows as it arrives", 2200);
  const text = dialog.getByRole("textbox", { name: "Your reply" });
  await text.waitFor();
  await caption(page, "Then the reply types itself in", 1800);
  await dialog.getByRole("button", { name: /^Send/u }).and(page.locator(":enabled")).waitFor();
  await caption(page, "The thinking folds away once the reply is written", 2000);
  await dialog.getByText("How Scout thought about it").click();
  await pause(1600);

  // Back to the choices, and combine two of them in plain words.
  await dialog.getByRole("button", { name: "Back to the choices" }).click();
  await dialog.getByRole("textbox", { name: "Or say it in your words" }).fill("1 and 3, warmer");
  await caption(page, "Say “1 and 3, warmer”: Scout knows which suggestions you mean", 2200);
  await dialog.getByRole("button", { name: "Draft it" }).click();
  await dialog.getByText("Scout is thinking…").waitFor();
  await pause(1200);
  await dialog.getByRole("button", { name: /^Send/u }).and(page.locator(":enabled")).waitFor();
  await caption(page, "One reply, from both suggestions", 2200);
  await page.keyboard.press("Escape");
  await pause(900);
}
