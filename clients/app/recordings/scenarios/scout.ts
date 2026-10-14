import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

import { caption, signedIn } from "./inbox.ts";

// Tell Scout what you care about, on the real stack (see inbox.ts for how to
// start it), with the `complete` role pointed at a stand-in model that knows
// a few demo sentences (CAMPBOOKS_AI__COMPLETE__BASE_URL /…__MODEL). The
// fake Google's demo mailbox is synced first, so the questions suggest real
// senders and every rule shows what it matches, counted by the server.
// Fictional people and mail only.
//
//   INBOX_BASE_URL=http://localhost:5181 pnpm --dir clients record scout \
//     --base-url http://localhost:5181

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

export default async function scout({ page }: ScenarioContext): Promise<void> {
  const shots = process.env["INBOX_SHOTS"];
  try {
    await record(page);
  } catch (error) {
    if (shots !== undefined) {
      await page.screenshot({ path: `${shots}/scout-failed.png` });
    }
    throw error;
  }
}

async function record(page: Page): Promise<void> {
  const workspace = await signedIn(page);
  const phone = isPhone(page);

  // Home offers the questions once mail is connected.
  await page.goto(`/w/${workspace}`);
  await page.getByRole("button", { name: "Start" }).waitFor();
  await caption(page, "Once mail is connected, Home offers three optional questions", 2000);
  await page.getByRole("button", { name: "Start" }).click();

  // 1 of 3: always reach me, from the people replied to most.
  await page.getByRole("heading", { name: "Whose mail should always reach you?" }).waitFor();
  await caption(page, "One question per view. S skips, Esc is Not now");
  await page.getByRole("checkbox", { name: "Maya Okafor" }).click();
  await page.getByText(/^Matches \d+ email/u).first().waitFor();
  await caption(page, "Each answer becomes a rule, with what it matches, counted by the server", 2400);
  await (phone
    ? page.getByRole("button", { name: /^Next/u }).click()
    : page.keyboard.press("Enter"));

  // 2 of 3: what never needs me.
  await page.getByRole("heading", { name: "What never needs you?" }).waitFor();
  await page.getByRole("checkbox", { name: "Newsletters and mailing lists" }).click();
  await page.getByText("Newsletters and mailing lists never need you.").waitFor();
  await pause(1600);
  await (phone
    ? page.getByRole("button", { name: /^Next/u }).click()
    : page.keyboard.press("Enter"));

  // 3 of 3: ask first, the default; skipped with S.
  await page.getByRole("heading", { name: "When Scout is sure, should it act?" }).waitFor();
  await caption(page, phone ? "Skip keeps Ask me first" : "S skips: Ask me first stays");
  await (phone
    ? page.getByRole("button", { name: /^Skip/u }).click()
    : page.keyboard.press("s"));

  // Review: per inbox, then Save with one Undo.
  await page.getByRole("heading", { name: "Here’s what Scout will do" }).waitFor();
  await caption(page, "Rules run before any AI and always give the same result", 2200);
  await (phone
    ? page.getByRole("button", { name: /^Save/u }).click()
    : page.keyboard.press("Enter"));
  await page.getByRole("heading", { name: "Scout knows what you care about" }).waitFor();
  await pause(1800);

  // Scout's memory: teach it in your own words.
  await page.goto(`/w/${workspace}/settings/memory`);
  const field = page.getByRole("textbox", { name: "What should Scout do?" });
  await field.waitFor();
  await caption(page, "Teach Scout in your own words: only the sentence is sent, never mail", 2200);
  await field.click();
  await field.pressSequentially("anything from my landlord Joana always matters", { delay: 35 });
  await (phone
    ? page.getByRole("button", { name: /^Teach/u }).click()
    : page.keyboard.press("Enter"));
  await page.getByText(/Who do you mean by/u).waitFor();
  await caption(page, "A name without an address is a question, never a guess", 2200);
  await page.getByRole("button", { name: /Joana Reis/u }).click();
  await page.getByText(/^Matches \d+ email/u).first().waitFor();
  await caption(page, "The rule it understood, with what it matches. Enter saves", 2400);
  await (phone
    ? page.getByRole("button", { name: /^Save rule/u }).click()
    : page.keyboard.press("Enter"));
  await page.getByText("Rule saved").waitFor();
  await pause(2200);

  // What rules can't do gets an honest answer.
  await field.fill("");
  await field.pressSequentially("forward every invoice to my accountant", { delay: 30 });
  await (phone
    ? page.getByRole("button", { name: /^Teach/u }).click()
    : page.keyboard.press("Enter"));
  await page.getByText(/Scout can’t send, forward or reply to mail/u).waitFor();
  await pause(2400);
}
