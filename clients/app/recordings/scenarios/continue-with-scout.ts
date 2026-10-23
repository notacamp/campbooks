import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Continue with Scout (campbooks-cloud#305, "Ask about this"), in Storybook
// with demo data only, at 375px and on the desktop:
//   pnpm --dir clients storybook
//   pnpm --dir clients record continue-with-scout --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

export default async function continueWithScout({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("screens-scout--continue-from-home-live"));
  const marta = page.getByText("Marta Ferreira is waiting for your reply");
  await marta.waitFor();
  await caption(page, "Home: Marta is waiting for a reply", 1600);
  await marta.click();
  await caption(page, "Continue with Scout, after the need's own actions (or .)", 2000);
  await page.getByRole("button", { name: /Continue with Scout/u }).click();
  await page.getByRole("button", { name: "Why is this here?" }).waitFor();
  await caption(page, "Scout opens on Marta's email, with questions to start from", 2200);
  await page.getByRole("button", { name: "Why is this here?" }).click();
  await caption(page, "It reads what happened to the email, then answers as it writes", 2600);
  await page.getByText(/It is under Reply because Marta asks you/u).waitFor({ timeout: 15_000 });
  await pause(1200);
  await page.getByText("What I looked at").click();
  await caption(page, "What I looked at: built from the tools that ran", 2400);
  const field = page.getByRole("textbox", { name: "Ask about this" });
  await field.click();
  await field.pressSequentially("When do the builders need to know?", { delay: 60 });
  await caption(page, "Ask more; in this version Scout changes nothing", 2000);
  await page.keyboard.press("Escape");
  await caption(page, "Esc closes it; the focus goes back to Continue with Scout", 2000);

  await page.goto(story("screens-scout--ask-in-the-reader-live"));
  await page.getByRole("heading", { name: /Q4 plan/u }).waitFor();
  await caption(page, "In the reader: Ask Scout, or .", 1800);
  await page.keyboard.press(".");
  await page.getByRole("button", { name: "Why is this here?" }).waitFor();
  await caption(page, "The chat about the open conversation", 2400);
}
