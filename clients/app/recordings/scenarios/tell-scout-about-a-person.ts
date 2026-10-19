import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Tell Scout about a person (campbooks-cloud#272) on the real stack: start
// it as reply-dialog.ts says (the fake `complete` model writes the drafts,
// with canned, fictional text), then
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record \
//     tell-scout-about-a-person --base-url http://localhost:5181
//
// Reply on the first email waiting for a reply (Marta's in the demo
// mailbox), tell Scout about the person, rewrite the draft with the note,
// then find the note in Scout's memory: edit it, remove it, and Undo.
// Fictional person and mail only; nothing is sent.

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

export default async function tellScoutAboutAPerson({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeFilled(page, workspace);
  await page.getByRole("group", { name: /^Actions for/u }).waitFor();

  await page
    .getByText(/is waiting for your reply/u)
    .first()
    .click();
  await (phone
    ? page
        .getByRole("button", { name: /^Reply/u })
        .first()
        .click()
    : page.keyboard.press("r"));
  const dialog = page.getByRole("dialog", { name: /^Reply to/u });
  await dialog.waitFor();
  await dialog.getByRole("button", { name: /Say yes/u }).waitFor();
  await (phone
    ? dialog.getByRole("button", { name: /Say yes/u }).click()
    : page.keyboard.press("1"));
  await dialog.getByRole("textbox", { name: "Your reply" }).waitFor();
  await caption(page, "Scout's draft, written the way you wrote to them before", 2000);

  const tell = dialog.getByRole("button", { name: /^Tell Scout about/u });
  await caption(page, "Under the quote: tell Scout who this is and how to write to them", 2200);
  await tell.click();
  const field = dialog.getByRole("textbox", { name: /and how to write to them$/u });
  await field.waitFor();
  await field.pressSequentially("My landlord. Formal, in Portuguese.", { delay: 40 });
  await caption(page, "Your words, private to you. Save with ⌘↵", 1800);
  await dialog.getByRole("button", { name: /^Save/u }).click();
  await dialog.getByText(/^Scout knows:/u).waitFor();
  await caption(page, "Scout knows it now. Nothing is rewritten until you ask", 2000);
  await dialog.getByRole("button", { name: /Rewrite with this/u }).click();
  await pause(2400);
  await caption(page, "The draft again, following your note", 1800);
  await page.keyboard.press("Escape");
  await pause(600);

  await page.goto(`/w/${workspace}/settings/memory`);
  const people = page.getByRole("list", { name: "People you told Scout about" });
  await people.waitFor();
  await people.scrollIntoViewIfNeeded();
  await caption(page, "Scout's memory lists the people you told it about", 2000);
  await people
    .getByRole("button", { name: /^Edit what Scout knows/u })
    .first()
    .click();
  const edit = people.getByRole("textbox", { name: /and how to write to them$/u });
  await edit.fill("My landlord. Formal, in Portuguese. Você, never tu.");
  await page.keyboard.press("Control+Enter");
  await pause(1400);
  await caption(page, "Edit it in place, or remove it", 1600);
  await people
    .getByRole("button", { name: /^Remove what Scout knows/u })
    .first()
    .click();
  await pause(1200);
  await caption(page, "Removed, with Undo", 1600);
  await page.getByRole("button", { name: /^Undo/u }).first().click();
  await pause(2000);
}
