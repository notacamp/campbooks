import { pause, type ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// To, Cc and Bcc suggest people as you type (campbooks-cloud#304), in
// Storybook with demo data only: a new message whose draft keeps what is
// typed, and fictional people to suggest.
//   pnpm --dir clients storybook
//   pnpm --dir clients record recipient-suggestions --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-inbox--${id}&viewMode=story`;

export default async function recipientSuggestions({ page }: ScenarioContext): Promise<void> {
  const phone = (page.viewportSize()?.width ?? 1440) < 768;
  await page.goto(story(phone ? "suggest-people-idle-phone" : "suggest-people-idle"));
  const composer = page.getByRole("region", { name: "New message" });
  const to = composer.getByRole("combobox", { name: "To" });
  await to.waitFor();
  await caption(page, "A new message: type a few letters of a name in To", 1800);

  await to.pressSequentially("mar", { delay: 160 });
  await composer.getByRole("listbox", { name: "Suggestions for To" }).waitFor();
  await caption(page, "The people you exchange mail with, written to most first", 2200);
  await page.keyboard.press("ArrowDown");
  await caption(page, phone ? "Down moves" : "↓ moves", 1100);
  await page.keyboard.press("Enter");
  await caption(page, phone ? "Enter adds Mário" : "↵ adds Mário", 1600);

  await composer.getByRole("button", { name: "Cc Bcc" }).click();
  const cc = composer.getByRole("combobox", { name: "Cc", exact: true });
  await cc.click();
  await cc.pressSequentially("mar", { delay: 160 });
  await composer.getByRole("listbox", { name: "Suggestions for Cc" }).waitFor();
  await caption(page, "In Cc too, without Mário: he is in To already", 2200);
  if (phone) {
    // A tap picks one.
    await composer.getByRole("option", { name: /Marta Ferreira/u }).click();
    await caption(page, "A tap adds Marta", 1800);
  } else {
    await page.keyboard.press("Enter");
    await caption(page, "↵ adds Marta", 1600);
    // Nobody suggested: what is typed is added as before.
    await cc.pressSequentially("new@studio.example", { delay: 60 });
    await pause(600);
    await page.keyboard.press("Enter");
    await caption(page, "A new address still works as before", 1800);
  }
  await pause(800);
}
