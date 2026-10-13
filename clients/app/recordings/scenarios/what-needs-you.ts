import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Home's "What needs you" on the real stack (see inbox.ts for how to start
// it): the fake Google's demo mailbox is synced and read by the rules, its
// needs are projected by the attention module, and the screens read them
// through the BFF. Fictional person and mail only.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record what-needs-you \
//     --base-url http://localhost:5181

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Wait until Home lists `count` things or more (the projection runs a moment after the sync). */
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

export default async function whatNeedsYou({ page }: ScenarioContext): Promise<void> {
  const shots = process.env["INBOX_SHOTS"];
  try {
    await record(page);
  } catch (error) {
    // Where it stopped, for whoever runs it.
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/what-needs-you-failed.png` });
    }
    throw error;
  }
}

async function record(page: Page): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);
  await homeFilled(page, workspace);
  await page.getByText("This may be phishing. Don’t use its links.").waitFor();
  await caption(
    page,
    "Home: what needs you, grouped Review, Reply, Do, Pay, due soonest first",
    2400,
  );
  await caption(
    page,
    "Production rules only: no model, no percentages, the sender's own words",
    2400,
  );

  if (!phone) {
    await caption(page, "J and K move; Enter opens");
    await page.keyboard.press("j");
    await pause(500);
    await page.keyboard.press("j");
    await pause(500);
    await page.keyboard.press("k");
    await pause(500);
  }

  // R replies: the conversation opens with the composer.
  await caption(
    page,
    phone ? "Reply opens the conversation's composer" : "R replies in the conversation",
  );
  if (phone) {
    await page.getByText("Marta Ferreira is waiting for your reply").click();
    await page
      .getByRole("button", { name: /^Reply/u })
      .first()
      .click();
  } else {
    await page.keyboard.press("r");
  }
  await page.getByRole("heading", { name: "Walkthrough on Thursday" }).waitFor();
  await pause(1800);
  await page.goto(`/w/${workspace}`);
  await page.getByText("Marta Ferreira is waiting for your reply").waitFor();

  // E done, Z undo.
  await caption(page, phone ? "Done takes it off your list" : "E marks it done");
  await page.getByText("Marta Ferreira is waiting for your reply").click();
  await pause(600);
  await (phone
    ? page.getByRole("button", { name: /^Done/u }).first().click()
    : page.keyboard.press("e"));
  await pause(1200);
  await caption(page, phone ? "Undo brings it back" : "Z brings it back");
  await (phone
    ? page.getByRole("button", { name: /^Undo/u }).first().click()
    : page.keyboard.press("z"));
  await pause(1500);

  // L later.
  await caption(page, phone ? "Later puts it off for you only" : "L puts it off for you only");
  const pedro = page.getByText("Pedro Alves asked you to do something");
  await pedro.click();
  await page
    .getByRole("button", { name: /^Later/u })
    .first()
    .click();
  await page.getByRole("dialog", { name: "Bring it back" }).waitFor();
  await pause(900);
  await page.getByRole("button", { name: "Monday" }).click();
  await pause(1500);

  // The phishing warning: its tells, then It's safe.
  await caption(page, "A warning names the sender by address and says why", 2000);
  await page.getByText("This may be phishing. Don’t use its links.").click();
  await pause(2000);
  await caption(page, "It's safe (X) takes it off and teaches Campbooks", 2000);
  await (phone
    ? page
        .getByRole("button", { name: /It’s safe/u })
        .first()
        .click()
    : page.keyboard.press("x"));
  await pause(1500);

  // Inbox chips and the reader's note.
  await page.goto(`/w/${workspace}/inbox/primary`);
  await page.getByText("Walkthrough on Thursday").first().waitFor();
  await caption(page, "The inbox carries the same need as chips on each row", 2400);
  await page.getByText("Walkthrough on Thursday").first().click();
  await page.getByRole("heading", { name: "Walkthrough on Thursday" }).waitFor();
  await caption(page, "The reader's note: the sentence, the sender's words, the chips", 2400);
  await page.getByText("How Campbooks decided").first().click();
  await caption(page, "How Campbooks decided: the only place the numbers show", 2600);
}
