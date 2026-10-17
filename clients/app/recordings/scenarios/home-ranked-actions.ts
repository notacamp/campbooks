import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Each need's own actions, ranked by the decide model (campbooks-cloud#166),
// on the real stack: start it as inbox.ts says, with the action catalogue on
// and a `decide` model configured, or with the demo ratings seeded (the
// steps are in the PR that added this scenario):
//
//   CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED=true \
//   CAMPBOOKS_CONVERSATIONS__ACTIONS__FLOOR_PERCENT=70 … cargo run -p campbooks-server -- all
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record home-ranked-actions \
//     --base-url http://localhost:5181
//
// Home lists buttons by name more than once (Not now, Snooze…), so every
// locator is scoped to a row's actions group or to the dialog. Fictional
// people and mail only; nothing is sent (Forward stops at the composer).

const PICKED = "Scout picked these for this email";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/**
 * Wait until Home shows actions Scout picked. The heading comes with the
 * attention list, the ranked actions a moment later with the conversations:
 * wait for the visor itself, and reload only when it doesn't come (the model
 * pass runs after the sync).
 */
async function picked(page: Page, workspace: string): Promise<void> {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    await page.goto(`/w/${workspace}`);
    await page.getByRole("heading", { level: 1 }).waitFor();
    try {
      await page.getByRole("img", { name: PICKED }).first().waitFor({ timeout: 5000 });
      return;
    } catch {
      await pause(1000);
    }
  }
  throw new Error("Home never showed actions Scout picked: is the catalogue on and rated?");
}

/** Select a row by its sender; its actions show under it. */
async function open(page: Page, name: string) {
  await page
    .getByRole("button", { name: new RegExp(name, "u") })
    .first()
    .click();
  const actions = page.getByRole("group", { name: `Actions for ${name}` });
  await actions.waitFor();
  return actions;
}

export default async function homeRankedActions({ page }: ScenarioContext): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await picked(page, workspace);
  await caption(page, "Each email now offers its own actions, picked by Scout", 2600);

  const marta = await open(page, "Marta Ferreira");
  await caption(page, "The visor says Scout picked them; likelier first, on 1, 2 and 3", 2600);

  // Remind asks when, in a dialog: the same times as Snooze.
  await marta.getByRole("button", { name: /^Remind me…/u }).click();
  const remind = page.getByRole("dialog", { name: /^Remind me about Marta Ferreira/u });
  await remind.waitFor();
  await caption(page, "Remind asks when: one decision", 2000);
  await remind.getByRole("button", { name: /Tomorrow morning/u }).click();
  await caption(page, "It folds away with a one-line confirmation; Undo stays", 2200);
  // The toast's Undo (a status region), not Done today's.
  await (phone
    ? page.getByRole("status").getByRole("button", { name: /^Undo/u }).first().click()
    : page.keyboard.press("z"));
  await pause(1500);

  // Forward asks to whom: found by name, never an address typed from memory.
  const again = await open(page, "Marta Ferreira");
  await again.getByRole("button", { name: /^Forward…/u }).click();
  const forward = page.getByRole("dialog", { name: /^Forward Marta Ferreira/u });
  await forward.waitFor();
  await caption(page, "Forward asks to whom: find them by name", 2000);
  await forward
    .getByRole("searchbox", { name: "Find someone by name" })
    .pressSequentially("Pedro", { delay: 120 });
  await pause(1200);
  await forward.getByRole("button", { name: /Pedro Alves/u }).click();
  // The composer, not the stream's heading: a phone shows the reader alone.
  await page.getByRole("heading", { level: 2, name: "Forward" }).waitFor();
  await caption(page, "The forward waits in the composer, addressed: nothing is sent yet", 2600);

  // Open attachment: the item opens on its file.
  await page.goto(`/w/${workspace}`);
  await page.getByRole("img", { name: PICKED }).first().waitFor();
  const pedro = await open(page, "Pedro Alves");
  await pedro.getByRole("button", { name: /^Open attachment/u }).click();
  await caption(page, "Open attachment opens the item on its file; Enter previews it", 2600);
  if (!phone) {
    await page.keyboard.press("Escape");
    await caption(page, "Without Scout's ranking, Home offers the same actions as before", 2400);
  }
}
