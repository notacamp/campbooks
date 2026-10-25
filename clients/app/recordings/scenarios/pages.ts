import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Pages (campbooks-cloud#363), in Storybook with demo data only: the page
// tree, a page opened, typing and a to-do made with `/`, a to-do ticked, the
// mentions (one the viewer cannot open), sharing, the trash with Undo and
// Restore; and on a phone, the tree, a page, the formatting bar and nothing
// scrolling sideways.
//   pnpm --dir clients storybook
//   pnpm --dir clients record pages --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-pages--${id}&viewMode=story`;

type Page = ScenarioContext["page"];

/** The editor's text box. */
const editorOf = (page: Page) => page.getByRole("textbox", { name: "Page text" });

/** Wait until the top bar says the page is saved. */
const savedOf = (page: Page) =>
  page.getByRole("status").filter({ hasText: "Saved" }).first().waitFor();

export default async function pages({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;

  if (phone) {
    await page.goto(story("the-tree-phone-idle"));
    await page.getByRole("button", { name: "Team handbook", exact: true }).waitFor();
    await caption(page, "On a phone the tree is its own screen", 2500);

    await page.getByRole("button", { name: "Team handbook", exact: true }).click();
    await editorOf(page).waitFor();
    await caption(page, "A page opens full screen", 2500);

    await editorOf(page).click();
    await caption(page, "The formatting bar sits above the keyboard", 2500);

    await page.mouse.wheel(0, 700);
    await caption(page, "Long pages scroll down, never sideways", 2500);
    await pause(500);
    return;
  }

  // The tree.
  await page.goto(story("the-tree-idle"));
  await page.getByRole("button", { name: "Team handbook", exact: true }).waitFor();
  await caption(page, "Pages: Workspace, Private and Shared with you", 2500);

  // Open a page.
  await page.getByRole("button", { name: "Team handbook", exact: true }).click();
  await editorOf(page).waitFor();
  await caption(page, "Open a page: headings, to-dos, a callout and lists", 2500);

  // Type a line at the end of the document, then make a to-do with `/`.
  const last = editorOf(page).getByText("Notes from the kickoff go below.");
  await last.scrollIntoViewIfNeeded();
  await last.click();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Pages save as you type.", { delay: 40 });
  await page.keyboard.press("Enter");
  await page.keyboard.type("/to-do", { delay: 90 });
  await caption(page, "Type / to pick a block", 2000);
  await page.keyboard.press("Enter");
  await page.keyboard.type("Call the venue", { delay: 50 });
  await savedOf(page);
  await caption(page, "A new to-do, made with /. Saved", 2200);

  // Tick a to-do.
  await editorOf(page).getByRole("checkbox").nth(1).check();
  await savedOf(page);
  await caption(page, "Tick a to-do", 2000);

  // Mentions.
  await editorOf(page).getByText("Jonas Weber").scrollIntoViewIfNeeded();
  await caption(page, "Mentions show the record's own name", 2500);
  await editorOf(page)
    .getByText(/Something you can.t open/u)
    .first()
    .scrollIntoViewIfNeeded();
  await caption(page, "A record you can't open says only that", 2500);

  // Share.
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByLabel("Person").selectOption({ label: "Rui Matos" });
  await page.getByLabel("Access").selectOption({ label: "Can edit" });
  await caption(page, "Share a page with Rui: can edit", 2500);
  await page.getByRole("button", { name: "Share", exact: true }).last().click();
  await caption(page, "Rui can now edit it", 2000);
  await page.keyboard.press("Escape");

  // Trash with Undo.
  await page
    .getByRole("button", { name: "Ideas for the Acme Studio kickoff", exact: true })
    .click();
  await page.getByRole("textbox", { name: "Page title" }).waitFor();
  await page.getByRole("button", { name: "More" }).click();
  await page.getByRole("button", { name: "Move to trash" }).click();
  await caption(page, "Move to trash: Undo is in the toast", 2500);

  // The trash, with Restore.
  await page.getByRole("button", { name: "Trash", exact: true }).click();
  await page.getByRole("button", { name: "Restore" }).first().waitFor();
  await caption(page, "The trash keeps pages for 30 days; Restore brings one back", 2500);
  await page.getByRole("button", { name: "Restore" }).first().click();
  await pause(1500);
}
