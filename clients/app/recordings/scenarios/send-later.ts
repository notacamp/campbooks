import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Sending a reply later (campbooks-cloud#392), in Storybook with demo data
// only: a reply draft open in its thread, kept in memory. Nothing is sent.
//   pnpm --dir clients storybook
//   pnpm --dir clients record send-later --base-url http://localhost:6006
const REPLY = "/iframe.html?id=screens-inbox--reply-draft-open&viewMode=story";
const DRAFTS = "/iframe.html?id=screens-inbox--drafts-scheduled-idle&viewMode=story";

/** A date a few days ahead, as a date input reads it (YYYY-MM-DD). */
function inDays(days: number): string {
  const at = new Date(Date.now() + days * 86_400_000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${String(at.getFullYear())}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
}

export default async function sendLater({ page }: ScenarioContext): Promise<void> {
  await run(page);
}

async function run(page: Page): Promise<void> {
  await page.goto(REPLY);
  const composer = page.getByRole("region", { name: /^Reply to/u });
  await composer.getByRole("textbox", { name: "Message" }).waitFor();
  await caption(page, "A reply draft: Send leaves now, its chevron sends later", 2200);

  // The menu: tomorrow morning, the next working day, a time of one's own.
  await composer.getByRole("button", { name: "Send later" }).click();
  await page.getByRole("button", { name: /Tomorrow morning/u }).waitFor();
  await caption(page, "Each choice shows the moment it means, in your time zone", 2600);

  // Pick a time: one that has passed is refused, and says why.
  await page.getByRole("button", { name: "Pick a time…" }).click();
  await page.getByLabel("Date").fill("2020-01-01");
  await page.getByRole("button", { name: "Schedule" }).click();
  await page.getByRole("alert").waitFor();
  await caption(page, "A time that has passed is turned down, with the reason", 2400);
  await page.getByLabel("Date").fill(inDays(3));
  await page.getByLabel("Time").fill("14:30");
  await caption(page, "Pick a date and a time", 1600);
  await page.getByRole("button", { name: "Schedule" }).click();

  // The composer closes with a short toast; Undo takes the send back.
  const toast = page.getByText(/^Scheduled for /u);
  await toast.waitFor();
  await caption(page, "It closes with “Scheduled for …” and an Undo", 2400);
  await page.getByRole("button", { name: /^Undo/u }).click();
  await composer.getByRole("textbox", { name: "Message" }).waitFor();
  await caption(page, "Undo cancels the send and brings the draft back", 2400);

  // The Drafts list keeps what is scheduled.
  await page.goto(DRAFTS);
  const group = page.getByRole("region", { name: "Scheduled" });
  await group.waitFor();
  await caption(page, "Scheduled sends wait in Drafts, with when they leave", 2600);
  await group.getByRole("button", { name: /^Cancel send:/u }).click();
  await page.getByText("Send cancelled").waitFor();
  await caption(page, "Cancel send turns one back into a draft", 2000);
  await pause(600);
}
