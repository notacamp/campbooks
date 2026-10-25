import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Microsoft mail on the connect screen (microsoft-mail-connection), in
// Storybook with demo data only: while the instance doesn't offer it an
// Outlook address says it's coming soon; once offered, Outlook, Hotmail,
// Live and Microsoft 365 addresses go to Microsoft's own sign-in.
//   pnpm --dir clients storybook
//   pnpm --dir clients record microsoft-mailbox --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-mailboxes--${id}&viewMode=story`;

export default async function microsoftMailbox({ page }: ScenarioContext): Promise<void> {
  const dialog = page.getByRole("dialog");
  const address = dialog.getByLabel("Email address");
  const type = async (value: string) => {
    await address.fill("");
    await address.pressSequentially(value, { delay: 40 });
    await dialog.getByRole("button", { name: /Continue/u }).click();
  };

  await page.goto(story("add-outlook-coming-soon-idle"));
  await address.waitFor();
  await caption(page, "Microsoft mail switched off on this instance", 1600);
  await type("ana@outlook.com");
  await dialog.getByText("Outlook is coming soon").waitFor();
  await caption(page, "An Outlook address says it's coming soon", 2000);

  await page.goto(story("add-outlook-idle"));
  await address.waitFor();
  await caption(page, "Microsoft mail switched on", 1600);
  await type("ana@outlook.com");
  await dialog.getByRole("button", { name: /Connect Outlook/u }).waitFor();
  await caption(page, "Outlook connects with Microsoft's own sign-in, no password", 2400);
  await dialog.getByRole("button", { name: "Back" }).click();

  await type("ana@hotmail.com");
  await dialog.getByRole("button", { name: /Connect Outlook/u }).waitFor();
  await caption(page, "Hotmail and Live go the same way", 2000);
  await dialog.getByRole("button", { name: "Back" }).click();

  await type("ana@contoso.example");
  await dialog.getByRole("button", { name: /Connect Outlook/u }).waitFor();
  await caption(page, "So does a Microsoft 365 work address", 2400);
}
