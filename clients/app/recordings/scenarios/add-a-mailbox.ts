import type { ScenarioContext } from "../context.ts";

import { caption } from "./inbox.ts";

// Add a mailbox by its address (imap-connect-screen), in Storybook with demo
// data only: the address picks the way in (Gmail's own sign-in, Outlook
// coming soon, an app password with the provider's steps), then the
// address is checked before sending turns on.
//   pnpm --dir clients storybook
//   pnpm --dir clients record add-a-mailbox --base-url http://localhost:6006
const story = (id: string) => `/iframe.html?id=screens-mailboxes--${id}&viewMode=story`;

export default async function addAMailbox({ page }: ScenarioContext): Promise<void> {
  const dialog = page.getByRole("dialog");
  const address = dialog.getByLabel("Email address");
  const type = async (value: string) => {
    await address.fill("");
    await address.pressSequentially(value, { delay: 40 });
    await dialog.getByRole("button", { name: /Continue/u }).click();
  };

  await page.goto(story("add-mailbox-idle"));
  await address.waitFor();
  await caption(page, "Add a mailbox starts with its address", 1800);

  await type("ana@gmail.com");
  await dialog.getByRole("button", { name: /Connect Gmail/u }).waitFor();
  await caption(page, "Gmail connects with Google's own sign-in", 2000);
  await dialog.getByRole("button", { name: "Back" }).click();

  await type("ana@outlook.com");
  await dialog.getByText("Outlook is coming soon").waitFor();
  await caption(page, "Outlook says it's coming soon", 1800);
  await dialog.getByRole("button", { name: "Back" }).click();

  await type("ana@yahoo.example");
  await dialog.getByRole("list", { name: "How to get an app password at Yahoo" }).waitFor();
  await caption(page, "Others get their provider's app-password steps", 2400);
  await dialog.getByText("Advanced: username and servers").click();
  await caption(page, "Advanced holds the servers discovery found", 2000);
  await dialog.getByText("Advanced: username and servers").click();
  await dialog
    .getByLabel("App password", { exact: true })
    .pressSequentially("abcd efgh ijkl mnop", { delay: 25 });
  await dialog.getByRole("button", { name: /^Connect/u }).click();
  await dialog.getByText("Checking your address…").waitFor();
  await caption(page, "Connected: Campbooks checks the address is really yours", 2200);
  await dialog.getByText("Verified", { exact: true }).waitFor({ timeout: 20_000 });
  await caption(page, "Verified, so sending turns on", 2000);

  await page.goto(story("add-not-verified-idle"));
  await address.waitFor();
  await type("ana@yahoo.example");
  await dialog.getByLabel("App password", { exact: true }).waitFor();
  await dialog
    .getByLabel("App password", { exact: true })
    .pressSequentially("abcd efgh ijkl mnop", { delay: 25 });
  await dialog.getByRole("button", { name: /^Connect/u }).click();
  await dialog
    .getByText("We couldn't verify this address: sending stays off")
    .waitFor({ timeout: 20_000 });
  await caption(page, "If the check fails, sending stays off", 2200);
  await dialog.getByRole("button", { name: "Try again" }).click();
  await caption(page, "Try again sends a new check", 1800);
  await dialog.getByText("Verified", { exact: true }).waitFor({ timeout: 20_000 });
  await caption(page, "Try again checks once more, and this time it is verified", 2400);
}
