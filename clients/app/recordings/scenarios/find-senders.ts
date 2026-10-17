import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Finding a sender by name in Scout's questions, in Storybook with demo data
// only:
//   pnpm --dir clients storybook
//   pnpm --dir clients record find-senders --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=${id}&viewMode=story`;

export default async function findSenders({ page }: ScenarioContext): Promise<void> {
  await page.goto(story("screens-scout-s-questions--always-reach-you"));
  await page.getByRole("heading", { name: "Whose mail should always reach you?" }).waitFor();
  await caption(page, "Scout suggests the ten people you reply to most", 2200);
  await page.getByRole("checkbox", { name: "Maya Okafor" }).click();
  await pause(600);

  const box = page.getByRole("searchbox", { name: "Find someone" });
  await box.click();
  await caption(page, "Anyone else: type part of a name, no address needed", 1800);
  await box.pressSequentially("joana", { delay: 140 });
  await page.getByRole("checkbox", { name: "Joana Reis" }).waitFor();
  await caption(page, "Scout finds her in your mail; Maya stays ticked", 2200);
  await page.getByRole("checkbox", { name: "Joana Reis" }).click();
  await page.getByText(/^Mail from Joana Reis/u).waitFor();
  await caption(page, "and shows the rule she becomes", 2200);

  await box.fill("");
  await box.pressSequentially("new@studio.example", { delay: 70 });
  await page.getByText("No mail from them yet").waitFor();
  await caption(page, "Someone new still works: a full address or a domain", 2200);
  await box.press("Enter");
  await pause(400);
  await caption(page, "Enter ticks the first match and clears the box", 2400);
}
