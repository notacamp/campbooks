import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Ask Scout from anywhere, and Recent (campbooks-cloud#305, phase 3), in
// Storybook with demo data only, at 375px and on the desktop:
//   pnpm --dir clients storybook
//   pnpm --dir clients record scout-chat-everywhere --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

const WAITING = "What am I waiting on?";

export default async function scoutChatEverywhere({ page }: ScenarioContext): Promise<void> {
  // ⌘K: a question about nothing in particular, asked at once.
  await page.goto(story("screens-scout--ask-from-command-menu-live"));
  await page.getByText("Marta Ferreira is waiting for your reply").waitFor();
  await caption(page, "Home. ⌘K asks Scout about anything", 1600);
  await page.keyboard.press("ControlOrMeta+k");
  const input = page.getByRole("combobox");
  await input.waitFor();
  await input.pressSequentially(WAITING, { delay: 45 });
  await caption(page, "Type a question: Ask Scout: <your words>", 2000);
  await page.getByRole("option", { name: `Ask Scout: ${WAITING}` }).click();
  await caption(page, "A new chat about your mail, asked straight away", 2200);
  await page
    .getByText(/You’re waiting on two people/u)
    .first()
    .waitFor({ timeout: 20_000 });
  await pause(1400);

  // Recent: back to yesterday's chat.
  await page.getByRole("button", { name: "Recent" }).click();
  const recent = page.getByRole("dialog", { name: "Recent" });
  await recent.getByText("Why is this here?").waitFor();
  await caption(page, "Recent: your chats, by your first question", 2200);
  await recent.getByText("Why is this here?").click();
  await page.getByText("Marta Ferreira · Walkthrough on Thursday").first().waitFor();
  await caption(page, "It reopens with its messages", 2200);
  await page.keyboard.press("Escape");
  await pause(600);

  // Scout noticed: ask why Scout suggests a rule.
  await page.goto(story("screens-scout--ask-from-scout-noticed-live"));
  const noticed = page.getByRole("region", { name: "Scout noticed" });
  await noticed.waitFor();
  await noticed.scrollIntoViewIfNeeded();
  await caption(page, "Scout noticed a sender you never open", 2000);
  await noticed.getByRole("button", { name: "More" }).click();
  await noticed.getByRole("button", { name: /^Ask Scout/u }).click();
  const why = page.getByRole("button", { name: "Why do you suggest this?" });
  await why.waitFor();
  await caption(page, "Ask Scout: the chat about that suggestion", 2200);
  await why.click();
  await page
    .getByText(/you opened 2 of them/u)
    .first()
    .waitFor({ timeout: 20_000 });
  await caption(page, "Scout explains, in numbers; nothing changes", 2400);
  await pause(800);
}
