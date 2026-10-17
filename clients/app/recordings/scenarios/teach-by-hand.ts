import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Building rules by hand in Scout's memory (campbooks-cloud#217), with no
// model call, in Storybook with demo data only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record teach-by-hand --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

export default async function teachByHand({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("screens-scout-s-memory--teach-by-hand"));
  const open = page.getByRole("button", { name: "Build a rule yourself" });
  await open.waitFor();
  await caption(page, "Scout’s memory: say it in a sentence, or build it yourself", 2200);
  await open.click();
  await page.getByRole("searchbox", { name: "Find someone" }).waitFor();
  await caption(page, "No sentence and no AI: pick which mail, then what Scout does", 2200);

  const box = page.getByRole("searchbox", { name: "Find someone" });
  await box.click();
  await box.pressSequentially("tax", { delay: 160 });
  await page.getByRole("checkbox", { name: "Joana Reis" }).waitFor();
  await caption(page, "Type part of a name: Scout finds them in your mail", 2000);
  await page.getByRole("checkbox", { name: "Joana Reis" }).click();
  await pause(600);

  await box.fill("");
  await box.pressSequentially("atelier-conseils.example", { delay: 70 });
  await page.getByText("Everyone at atelier-conseils.example").waitFor();
  await caption(page, "A whole domain works too: Enter ticks it", 2000);
  await box.press("Enter");
  await pause(600);

  await page
    .getByRole("combobox", { name: /What to do/u })
    .selectOption({ label: "Keep it in my inbox" });
  await caption(page, "Always reaches you: it stays in the inbox", 1800);
  await page.getByRole("button", { name: /Show the rules/u }).click();
  await page.getByText("One rule for each sender, so mail from any of them counts.").waitFor();
  await caption(page, "Two senders, two rules: mail from either one counts", 2400);

  await page
    .getByRole("button", { name: /Save rule/u })
    .first()
    .click();
  await pause(900);
  await page
    .getByRole("button", { name: /Save rule/u })
    .first()
    .click();
  await page.getByRole("searchbox", { name: "Find someone" }).waitFor();
  await caption(page, "Both saved, each with Undo, and the builder is ready again", 2400);
}
