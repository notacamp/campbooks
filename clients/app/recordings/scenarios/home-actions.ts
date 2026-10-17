import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Home's labelled actions (campbooks-cloud#165) on the real stack: start it
// as inbox.ts says, with a `decide` model configured (CAMPBOOKS_AI__DECIDE__*),
// since without one no need is raised (ADR 0018). Fictional person and mail
// only; nothing is sent.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record home-actions \
//     --base-url http://localhost:5181

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

export default async function homeActions({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeFilled(page, workspace);
  await page.getByRole("group", { name: /^Actions for/u }).waitFor();
  await caption(
    page,
    "The selected item shows what you can do with it, each button with its key",
    2600,
  );

  if (phone) {
    // Tap a row: its actions show under it; Snooze opens a bottom sheet.
    await page.getByText("Marta Ferreira").first().click();
    await caption(page, "Tap an item: its actions show right under it", 2000);
    await page
      .getByRole("button", { name: /^Snooze/u })
      .first()
      .click();
    await page.getByRole("dialog").waitFor();
    await caption(page, "Snooze is a bottom sheet: one decision", 2000);
    await page.getByRole("button", { name: /Tomorrow morning/u }).click();
    await caption(page, "The row folds away with a one-line confirmation; Undo stays", 2200);
    await page.getByRole("button", { name: /^Undo/u }).first().click();
    await pause(1500);
    await page.getByText("Pedro Alves").first().click();
    await page.getByRole("button", { name: /^Done/u }).first().click();
    await caption(page, "Done · Pedro Alves, then the next item", 2000);
    return;
  }

  // Arrows: ↓ moves between items, → walks the buttons, Enter presses.
  await caption(page, "↓ and ↑ move between items, as J and K do");
  await page.keyboard.press("ArrowDown");
  await pause(700);
  await page.keyboard.press("ArrowDown");
  await pause(700);
  await caption(page, "→ and ← move along the item's buttons");
  await page.keyboard.press("ArrowRight");
  await pause(600);
  await page.keyboard.press("ArrowRight");
  await pause(600);
  await page.keyboard.press("ArrowRight");
  await pause(600);
  await caption(page, "Esc clears the highlight");
  await page.keyboard.press("Escape");
  await pause(800);

  // S: the Snooze dialog, Esc changes nothing, 2 picks tomorrow.
  await caption(page, "S opens Snooze: one decision, each choice with its number");
  await page.keyboard.press("s");
  await page.getByRole("dialog").waitFor();
  await pause(1600);
  await caption(page, "Esc closes it with nothing changed");
  await page.keyboard.press("Escape");
  await pause(1000);
  await page.keyboard.press("s");
  await page.getByRole("dialog").waitFor();
  await pause(600);
  await page.keyboard.press("2");
  await caption(page, "The row turns into a one-line confirmation and folds away", 1800);
  await caption(page, "The toast keeps Undo; Z brings it back");
  await page.keyboard.press("z");
  await pause(1500);

  // E: done, with the confirmation; the next item takes the selection.
  await caption(page, "E marks it done; the next item is selected at once");
  await page.keyboard.press("e");
  await pause(1500);
  await page.keyboard.press("z");
  await pause(1200);

  // The ? sheet keeps every key now that the aside has no legend.
  await caption(page, "Every key is in the ? sheet");
  await page.keyboard.press("?");
  await pause(2200);
  await page.keyboard.press("Escape");
  await pause(600);

  // Pin, in the inbox: pinned conversations stay at the top of their stream.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByRole("heading", { level: 1, name: "Primary" }).waitFor();
  await caption(page, "In the inbox, Star is now Pin (P)");
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("p");
  await caption(page, "A pinned conversation stays at the top of its stream", 2400);
  await page.keyboard.press("z");
  await pause(1200);
}
